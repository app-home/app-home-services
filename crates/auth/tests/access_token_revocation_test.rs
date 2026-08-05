use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use axum::extract::FromRequestParts;
use axum::http::Request;
use uuid::Uuid;

use auth::adapters::jwt_service::JwtServiceImpl;
use auth::application::ports::jwt_service::JwtService;
use shared::auth::{AuthenticatedUser, JwtVerification};
use shared::ports::{AccessTokenBlacklist, BlacklistError};

const SECRET: &str = "test-secret-key-that-is-long-enough-for-hmac";
const ISSUER_AUDIENCE: &str = "app-home-services";

fn create_service() -> JwtServiceImpl {
    JwtServiceImpl::new(SECRET, 15, 7, ISSUER_AUDIENCE, ISSUER_AUDIENCE)
}

#[derive(Default)]
struct MockBlacklist {
    revoked: Mutex<HashSet<Uuid>>,
    fail_checks: bool,
}

#[async_trait::async_trait]
impl AccessTokenBlacklist for MockBlacklist {
    async fn revoke(&self, jti: Uuid, _ttl_secs: u64) -> Result<(), BlacklistError> {
        self.revoked.lock().unwrap().insert(jti);
        Ok(())
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        if self.fail_checks {
            return Err(BlacklistError);
        }
        Ok(self.revoked.lock().unwrap().contains(&jti))
    }
}

async fn build_parts(
    token: &str,
    blacklist: Arc<dyn AccessTokenBlacklist>,
) -> axum::http::request::Parts {
    let mut parts = Request::builder()
        .header("Authorization", format!("Bearer {token}"))
        .body(())
        .unwrap()
        .into_parts()
        .0;

    parts.extensions.insert(Arc::new(JwtVerification::new(
        SECRET,
        ISSUER_AUDIENCE.to_string(),
        ISSUER_AUDIENCE.to_string(),
    )));
    parts.extensions.insert(blacklist);
    parts
}

#[tokio::test]
async fn extractor_accepts_valid_not_revoked_token() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let pair = service
        .generate_token_pair(user_id, Uuid::now_v7())
        .unwrap();

    let mut parts = build_parts(&pair.access_token, Arc::new(MockBlacklist::default())).await;
    let user = AuthenticatedUser::from_request_parts(&mut parts, &())
        .await
        .unwrap();

    assert_eq!(user.user_id, user_id);
    assert!(!user.jti.is_nil());
    assert!(user.exp > 0);
}

#[tokio::test]
async fn extractor_rejects_revoked_token() {
    let service = create_service();
    let pair = service
        .generate_token_pair(Uuid::now_v7(), Uuid::now_v7())
        .unwrap();
    let claims = service.validate_access_token(&pair.access_token).unwrap();

    let blacklist = MockBlacklist {
        revoked: Mutex::new(HashSet::from([claims.jti])),
        fail_checks: false,
    };
    let mut parts = build_parts(&pair.access_token, Arc::new(blacklist)).await;

    assert!(
        AuthenticatedUser::from_request_parts(&mut parts, &())
            .await
            .is_err(),
        "a revoked access token must be rejected with 401 (see #88)"
    );
}

#[tokio::test]
async fn extractor_does_not_leak_revocation_between_tokens() {
    let service = create_service();
    let pair = service
        .generate_token_pair(Uuid::now_v7(), Uuid::now_v7())
        .unwrap();
    let other_pair = service
        .generate_token_pair(Uuid::now_v7(), Uuid::now_v7())
        .unwrap();
    let revoked_claims = service.validate_access_token(&pair.access_token).unwrap();

    let blacklist = MockBlacklist {
        revoked: Mutex::new(HashSet::from([revoked_claims.jti])),
        fail_checks: false,
    };
    let mut parts = build_parts(&other_pair.access_token, Arc::new(blacklist)).await;

    assert!(
        AuthenticatedUser::from_request_parts(&mut parts, &())
            .await
            .is_ok(),
        "revoking one token must not affect other tokens (see #88)"
    );
}

#[tokio::test]
async fn extractor_fails_open_when_blacklist_backend_errors() {
    let service = create_service();
    let pair = service
        .generate_token_pair(Uuid::now_v7(), Uuid::now_v7())
        .unwrap();

    let mut parts = build_parts(
        &pair.access_token,
        Arc::new(MockBlacklist {
            revoked: Mutex::new(HashSet::new()),
            fail_checks: true,
        }),
    )
    .await;

    assert!(
        AuthenticatedUser::from_request_parts(&mut parts, &())
            .await
            .is_ok(),
        "an unavailable revocation backend must fail open (see #88)"
    );
}

#[tokio::test]
async fn extractor_rejects_expired_token() {
    // Built directly with jsonwebtoken (not JwtServiceImpl, which always mints a
    // token expiring in the future) so `exp` can be set in the past. Even though
    // this token was never revoked, jsonwebtoken's own exp check must reject it
    // before the blacklist is ever consulted -- expiry and revocation are
    // independent rejection paths, and only the latter is exercised by the
    // tests above.
    //
    // `exp` is set 120s in the past, not 60s: jsonwebtoken's `Validation`
    // applies a default 60-second leeway around `exp`, so a token exactly 60s
    // expired would sit right on that boundary and could pass or fail
    // depending on the exact instant `now` is captured vs. when `decode` runs.
    // 120s clears the leeway with margin.
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": Uuid::now_v7(),
        "jti": Uuid::now_v7(),
        "iss": ISSUER_AUDIENCE,
        "aud": ISSUER_AUDIENCE,
        "exp": now - 120,
        "iat": now - 1020,
    });
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap();

    let mut parts = build_parts(&token, Arc::new(MockBlacklist::default())).await;

    assert!(
        AuthenticatedUser::from_request_parts(&mut parts, &())
            .await
            .is_err(),
        "an expired access token must be rejected with 401, independent of the revocation list"
    );
}
