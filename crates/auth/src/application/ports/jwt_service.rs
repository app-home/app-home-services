use uuid::Uuid;

use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

/// `jti`/`iss`/`aud` (added by #87/#88) are all required, non-optional fields,
/// which is a deliberate rollout tradeoff: any access token minted *before*
/// these fields existed fails deserialization here and is rejected outright,
/// rather than being accepted as a legacy/best-effort token. In practice this
/// means every access token still outstanding at deploy time -- for any
/// currently-logged-in user, not just ones whose token happens to expire soon
/// -- stops validating the moment this version starts serving requests, and
/// each of those users needs to log in again to get a compliant token. Refresh
/// tokens (`RefreshTokenClaims` below) are unaffected and still validate
/// old sessions through to a fresh access token, but only if they weren't
/// *also* minted before #87 (an old refresh token has the same iss/aud gap).
/// Accepted as a one-time, deploy-day cost in exchange for never having to
/// carry legacy-shaped-claims handling in the verifier going forward.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccessTokenClaims {
    pub sub: Uuid,
    /// Unique token id, minted per access token, so a single token can be
    /// revoked without affecting the others a user may hold (see #88).
    pub jti: Uuid,
    pub iss: String,
    pub aud: String,
    /// Unix seconds. `i64` (chrono's native type) rather than `usize` so the
    /// timestamp arithmetic has no platform-width/overflow surprises on 32-bit
    /// targets (Y2038) or for pre-1970 values (see #95).
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: Uuid,
    pub session_id: Uuid,
    pub iss: String,
    pub aud: String,
    /// Unix seconds; see `AccessTokenClaims::exp` for why `i64` (see #95).
    pub exp: i64,
    pub iat: i64,
}

pub trait JwtService: Send + Sync {
    fn generate_token_pair(&self, user_id: Uuid, session_id: Uuid) -> Result<TokenPair, AuthError>;
    fn validate_access_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError>;
    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenClaims, AuthError>;
}
