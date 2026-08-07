// Integration tests for migration 012 (`access_token_revocation_outbox
// expires at repair`): the self-healing guard that recovers the `expires_at`
// index when migration 011's CREATE INDEX CONCURRENTLY (or a REINDEX retry)
// failed partway and left an invalid index behind.
//
// These tests simulate the failure the migration exists for, then run the
// migration's DO block verbatim (the migration file is embedded via
// include_str!) against the shared outbox table:
//
//   - a failed CREATE INDEX CONCURRENTLY leaves `..._idx_test_*` invalid, and
//     the repair rebuilds it valid;
//   - the state a failed REINDEX CONCURRENTLY can leave (a `_ccnew`/`_ccold`
//     leftover, possibly numbered `_ccnewN`, plus an invalid target) is
//     manufactured directly, since PG 17 auto-cleans `_ccnew` on the ordinary
//     duplicate-key failure -- the leftover only persists for crash/phase-3
//     failures that cannot be triggered deterministically in a test;
//   - a valid index is left untouched (no rebuild, same relation OID).
//
// The index name is suffixed so the test never collides with the real
// `access_token_revocation_outbox_expires_at_idx`, and the migration SQL is
// string-substituted to point at that suffixed name. Suffixes must stay short:
// PostgreSQL silently truncates identifiers at 63 characters, and the repair's
// `_cc(new|old)` regex would never match a truncated leftover (e.g. `_ccne`).
// The real production index name (45 chars + `_ccnew`) is well under the limit.
//
// To run: cargo test -- --ignored migration_recovery
//
// Prerequisites: DATABASE_URL set and migrations applied (same as the other
// integration tests; see `access_token_revocation_outbox_test.rs`).

use sqlx::PgPool;

const TARGET_INDEX: &str = "access_token_revocation_outbox_expires_at_idx";
const TEST_INDEX_SUFFIXES: [&str; 3] = ["rebuild", "leftover", "valid"];
const REPAIR_MIGRATION: &str =
    include_str!("../../migrations/012_access_token_revocation_outbox_expires_at_repair.sql");

/// (relation oid, indisvalid) for a public-schema index, or `None` if absent.
async fn index_state(pool: &PgPool, name: &str) -> Option<(i64, bool)> {
    sqlx::query_as::<_, (i64, bool)>(
        "SELECT c.oid::bigint, i.indisvalid \
         FROM pg_class c \
         JOIN pg_index i ON i.indexrelid = c.oid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'public' AND c.relname = $1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .expect("index state lookup should succeed")
}

/// Runs migration 012's DO block, with its index name pointed at `test_index`.
async fn run_repair(pool: &PgPool, test_index: &str) {
    let sql = REPAIR_MIGRATION.replace(TARGET_INDEX, test_index);
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .expect("the repair migration's DO block should execute");
}

/// Inserts two outbox rows with the same `expires_at`, then tries to build a
/// UNIQUE index over it concurrently -- which must fail and leave an invalid
/// index behind, just like a cancelled concurrent build.
async fn leave_invalid_index(pool: &PgPool, test_index: &str) {
    sqlx::query(
        "INSERT INTO access_token_revocation_outbox (jti, ttl_secs, expires_at) \
         VALUES (gen_random_uuid(), 60, NOW()), (gen_random_uuid(), 60, NOW())",
    )
    .execute(pool)
    .await
    .expect("inserting duplicate outbox rows should succeed");

    let create = format!(
        "CREATE UNIQUE INDEX CONCURRENTLY {test_index} ON access_token_revocation_outbox (expires_at)"
    );
    let result = sqlx::raw_sql(sqlx::AssertSqlSafe(create))
        .execute(pool)
        .await;
    assert!(
        result.is_err(),
        "a unique index build over duplicate rows must fail, leaving an invalid index"
    );
}

/// Names of every public-schema index whose name starts with `prefix`
/// (position = 1 avoids LIKE's underscore wildcard doing prefix matching).
async fn indexes_with_prefix(pool: &PgPool, prefix: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT c.relname \
         FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'public' AND position($1::text IN c.relname) = 1",
    )
    .bind(prefix)
    .fetch_all(pool)
    .await
    .expect("listing indexes with a prefix should succeed")
}

async fn drop_test_indexes(pool: &PgPool, test_index: &str) {
    // The test index plus any numbered `_ccnew`/`_ccold` leftovers (e.g.
    // `_ccnew3`) that a previous, possibly interrupted run left behind.
    let names = indexes_with_prefix(pool, test_index).await;
    for name in names {
        let drop = format!("DROP INDEX IF EXISTS {name}");
        sqlx::raw_sql(sqlx::AssertSqlSafe(drop))
            .execute(pool)
            .await
            .expect("dropping the test index should succeed");
    }
}

/// Drops the indexes every migration test can leave behind (from any earlier,
/// possibly failed, run of any of them). A leftover -- even a valid, unique
/// one -- on the shared outbox table would block another test's duplicate-row
/// insert or index build.
async fn clean_previous_test_indexes(pool: &PgPool) {
    for suffix in TEST_INDEX_SUFFIXES {
        drop_test_indexes(pool, &format!("{TARGET_INDEX}_{suffix}")).await;
    }
}

#[test]
#[ignore]
fn repair_rebuilds_an_invalid_index_left_by_a_failed_concurrent_build() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        // Drop every test-prefixed index first: a leftover (possibly valid,
        // unique) index from an interrupted earlier run would block this test's
        // duplicate-row insert or index build.
        clean_previous_test_indexes(pool).await;
        let test_index = format!("{TARGET_INDEX}_rebuild");

        leave_invalid_index(pool, &test_index).await;
        assert_eq!(
            index_state(pool, &test_index).await.map(|(_, valid)| valid),
            Some(false),
            "the simulated failed build must leave the index invalid"
        );

        sqlx::query("DELETE FROM access_token_revocation_outbox")
            .execute(pool)
            .await
            .expect("clearing the duplicate rows should succeed");

        run_repair(pool, &test_index).await;

        assert_eq!(
            index_state(pool, &test_index).await.map(|(_, valid)| valid),
            Some(true),
            "the repair must rebuild the invalid index"
        );

        drop_test_indexes(pool, &test_index).await;
    })
}

#[test]
#[ignore]
fn repair_drops_a_ccnew_leftover_and_rebuilds_the_invalid_target() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        clean_previous_test_indexes(pool).await;
        let test_index = format!("{TARGET_INDEX}_leftover");

        // Manufacture the state a failed REINDEX CONCURRENTLY can leave: an
        // invalid target plus an invalid `_ccnew` leftover. A failed concurrent
        // unique build reliably leaves its own target invalid.
        leave_invalid_index(pool, &test_index).await;
        let ccnew = format!("{test_index}_ccnew");
        leave_invalid_index(pool, &ccnew).await;

        assert_eq!(
            index_state(pool, &test_index).await.map(|(_, valid)| valid),
            Some(false),
            "the failed reindex must leave the target index invalid"
        );
        // The leftover is either `<target>_ccnew` or a numbered variant
        // (`_ccnew3`, ...) when earlier interrupted runs left names behind.
        let leftover_prefix = format!("{test_index}_cc");
        let leftovers = indexes_with_prefix(pool, &leftover_prefix).await;
        assert!(
            !leftovers.is_empty(),
            "the failed reindex must leave an invalid _ccnew leftover, got: {leftovers:?}"
        );
        for name in &leftovers {
            assert_eq!(
                index_state(pool, name).await.map(|(_, valid)| valid),
                Some(false),
                "every _ccnew leftover must be invalid"
            );
        }

        sqlx::query("DELETE FROM access_token_revocation_outbox")
            .execute(pool)
            .await
            .expect("clearing the duplicate rows should succeed");

        run_repair(pool, &test_index).await;

        assert!(
            indexes_with_prefix(pool, &leftover_prefix).await.is_empty(),
            "the repair must drop every invalid _ccnew leftover"
        );
        assert_eq!(
            index_state(pool, &test_index).await.map(|(_, valid)| valid),
            Some(true),
            "the repair must rebuild the target index after cleaning the leftover"
        );

        drop_test_indexes(pool, &test_index).await;
    })
}

#[test]
#[ignore]
fn repair_leaves_a_valid_index_untouched() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        clean_previous_test_indexes(pool).await;
        let test_index = format!("{TARGET_INDEX}_valid");

        let create =
            format!("CREATE INDEX {test_index} ON access_token_revocation_outbox (expires_at)");
        sqlx::raw_sql(sqlx::AssertSqlSafe(create))
            .execute(pool)
            .await
            .expect("creating a valid test index should succeed");
        let (oid_before, valid_before) = index_state(pool, &test_index)
            .await
            .expect("the valid test index must exist");

        run_repair(pool, &test_index).await;

        let (oid_after, valid_after) = index_state(pool, &test_index)
            .await
            .expect("the valid test index must still exist");
        assert_eq!(
            (oid_after, valid_after),
            (oid_before, valid_before),
            "the repair must not rebuild (or otherwise touch) a valid index"
        );

        drop_test_indexes(pool, &test_index).await;
    })
}
