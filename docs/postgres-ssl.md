# Postgres SSL/TLS

## `sslmode` modes

The service parses `DATABASE_URL` before it passes the resulting connection URL to
sqlx. When `DB_REQUIRE_SSL=true`, it rewrites `sslmode` to `verify-full` (see
below). Otherwise the URL is passed through as given, and sqlx reads the `sslmode`
query parameter the same way libpq does:

| Mode | Behavior | Verdict |
|------|----------|---------|
| `disable` | No TLS at all. | OK **only** for a loopback host (see below). |
| `prefer` | Try TLS, silently fall back to plaintext. | This is sqlx's default when `sslmode` is omitted. Not a security guarantee. |
| `require` | Always TLS, but does **not** verify the server certificate. | Protects against passive sniffing, not against man-in-the-middle. |
| `verify-ca` | TLS + server certificate signed by a trusted CA. | Good. |
| `verify-full` | TLS + CA check + hostname match. | **Recommended** for any remote/production database. |

## Guardrail: no plaintext to non-loopback hosts

The service refuses to start if `DATABASE_URL` uses `sslmode=disable` against a
non-loopback database host (anything other than `127.0.0.1`, `::1` or
`localhost`). Sending credentials and data -- password hashes, user data, tokens
-- unencrypted over a real network is a fatal configuration error, so we fail
fast at startup rather than stream plaintext at runtime. See #85.

A connection that *can* silently fall back to plaintext (`sslmode` omitted, or
`sslmode=prefer`) against a non-loopback host is accepted but logged as a
startup warning recommending `sslmode=verify-full`.

## `DB_REQUIRE_SSL`

Set `DB_REQUIRE_SSL=true` to force the connection to `sslmode=verify-full`,
regardless of what `DATABASE_URL` itself says: the service rewrites the URL,
replacing any existing `sslmode` value (including an accidental
`sslmode=disable`). This is the explicit "production demands an encrypted,
certificate-verified connection" switch.

Note: `verify-full` requires the server certificate to be signed by a trusted CA
and its hostname to match, so `DB_REQUIRE_SSL=true` will **not** connect to a
database presenting a self-signed certificate. Leave it unset for local
development.

## Local development

The default development setup (`run-postgres-dev.ps1`, `compose.yaml`) runs
Postgres on `127.0.0.1`, so `sslmode=disable` there never leaves the machine and
is accepted by the guardrail above. `.env.example` documents the modes; keep any
`.env` that binds to `SERVER_HOST=0.0.0.0` pointed at a loopback database (or use
TLS) so the same file can never be reused against production.
