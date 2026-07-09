# Redis security: auth and TLS

## Auth (password-protected connections)

`RedisRateLimiter::connect` accepts a standard `redis://:password@host:port` URL
(passed straight through to the `redis` crate's `Client::open`). This is exercised by
`tests/integration/redis_auth_test.rs`:

- `connects_successfully_with_the_correct_password`
- `rejects_connection_with_the_wrong_password`
- `rejects_connection_with_no_password_at_all`

Run with: `cargo test --test integration -- --ignored redis_auth`

**Recommendation**: always set `--requirepass` (or Redis 6+ ACLs, if you need
per-application credentials rather than one shared password) on any Redis instance
this service talks to outside of local development, and put the resulting password in
`REDIS_URL` per `.env.example`.

## TLS: decision

**Decision: not handled at the `redis` crate level today. Redis is expected to run on
the same private network as the app (the same trust boundary Postgres already runs
in, in both `compose.yaml` and `run-postgres-dev.ps1`), with no TLS termination in
front of it.**

### Why

- The `redis` crate's native TLS support (`rediss://`) requires enabling one of its
  TLS Cargo features (`tokio-native-tls-comp` or `tokio-rustls-comp`), which isn't
  enabled in `Cargo.toml` today -- `rediss://` URLs would currently fail to connect.
- Setting this up meaningfully (a real cert, a Redis build with TLS listening
  enabled, and a test that actually exercises the handshake) is a nontrivial chunk of
  infrastructure for a service that, as configured in this repo today
  (`compose.yaml`, `run-postgres-dev.ps1`), only ever reaches Redis over a private
  Docker/Podman network -- the same network Postgres also isn't TLS-protected on.
- Adding a Cargo feature and cert-handling code for a threat model (an attacker
  positioned to sniff traffic *inside* that private network) that doesn't reflect how
  this service is actually deployed today would be complexity without a
  corresponding benefit yet.

### When to revisit this

Enable crate-native TLS (or decide on sidecar/proxy-terminated TLS instead, e.g. a
`stunnel` sidecar or a managed Redis provider that terminates TLS for you, in which
case this app would just speak plain `redis://` to `localhost` and never see
`rediss://` at all) once any of the following becomes true:

- Redis moves to a managed provider reachable over a network you don't fully
  control (e.g. a cloud Redis service reachable over the public internet, even if
  password-protected).
- The deployment target has a compliance requirement for encryption-in-transit
  between internal services.
- Redis and the app stop sharing the same private network/trust boundary for any
  other reason (e.g. Redis moves to a separate cluster/VPC).

If/when that happens: enable the appropriate `redis` crate TLS feature in
`Cargo.toml`, update `.env.example`'s `REDIS_URL` guidance, and add an `#[ignore]`d
integration test analogous to `redis_auth_test.rs` that stands up a
TLS-terminating Redis (a self-signed cert is fine for the test) and connects via
`rediss://`.
