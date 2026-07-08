use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::application::ports::auth_provider::{AuthProvider, GoogleUserInfo};
use crate::domain::errors::AuthError;

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

/// Fallback TTL used when Google's response doesn't include a parseable
/// Cache-Control max-age. Google documents that these keys rotate infrequently, so an
/// hour is a conservative, safe default between the two failure modes (too short
/// wastes the point of caching; too long risks serving a since-rotated key set).
const DEFAULT_JWKS_TTL: Duration = Duration::from_secs(3600);

/// Bounded wait for the JWKS fetch, so a hung/slow response from Google can't block a
/// login indefinitely (previously unbounded, limited only by OS/TCP defaults).
const JWKS_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct CachedJwks {
    jwks: jsonwebtoken::jwk::JwkSet,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct GoogleAuthProvider {
    client_id: String,
    /// A single shared client, built once, so TCP connections/TLS sessions to
    /// Google's certs endpoint can be pooled and reused across requests instead of
    /// paying fresh connection setup on every login.
    http_client: reqwest::Client,
    jwks_cache: Arc<RwLock<Option<CachedJwks>>>,
}

impl GoogleAuthProvider {
    pub fn new(client_id: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(JWKS_FETCH_TIMEOUT)
            .build()
            .expect("failed to build reqwest client for Google JWKS fetches");

        Self {
            client_id,
            http_client,
            jwks_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the current JWKS, serving from cache when still fresh and only
    /// hitting the network on a cache miss/expiry.
    async fn get_jwks(&self) -> Result<jsonwebtoken::jwk::JwkSet, AuthError> {
        if let Some(cached) = self.jwks_cache.read().await.as_ref() {
            if cached.expires_at > Instant::now() {
                return Ok(cached.jwks.clone());
            }
        }

        match self.fetch_jwks().await {
            Ok(fresh) => {
                let jwks = fresh.jwks.clone();
                *self.jwks_cache.write().await = Some(fresh);
                Ok(jwks)
            }
            // On a fetch failure, prefer serving a still-cached (even if just-expired)
            // JWKS over failing the login outright -- a transient Google outage or
            // rate limit shouldn't block every login while we still hold keys that
            // are very likely still valid. Only propagate the error when there's
            // nothing cached to fall back on.
            Err(e) => {
                if let Some(stale) = self.jwks_cache.read().await.as_ref() {
                    tracing::warn!(
                        error = %e,
                        "Failed to refresh Google JWKS; serving stale cached keys"
                    );
                    return Ok(stale.jwks.clone());
                }
                Err(e)
            }
        }
    }

    async fn fetch_jwks(&self) -> Result<CachedJwks, AuthError> {
        let response = self
            .http_client
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .map_err(|_e| AuthError::TokenVerificationFailed)?;

        let ttl = response
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_max_age)
            .unwrap_or(DEFAULT_JWKS_TTL);

        let jwks: jsonwebtoken::jwk::JwkSet = response
            .json()
            .await
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        Ok(CachedJwks {
            jwks,
            expires_at: Instant::now() + ttl,
        })
    }
}

/// Parses the `max-age=<seconds>` directive out of a `Cache-Control` header value, if
/// present. Pulled out as a standalone function so it can be unit-tested directly
/// against header strings, without needing a real HTTP response.
fn parse_max_age(cache_control: &str) -> Option<Duration> {
    cache_control.split(',').find_map(|directive| {
        let (name, value) = directive.trim().split_once('=')?;
        if name.eq_ignore_ascii_case("max-age") {
            value.trim().parse::<u64>().ok().map(Duration::from_secs)
        } else {
            None
        }
    })
}

#[async_trait]
impl AuthProvider for GoogleAuthProvider {
    async fn verify_id_token(&self, token: &str) -> Result<GoogleUserInfo, AuthError> {
        let jwks = self.get_jwks().await?;

        let header =
            jsonwebtoken::decode_header(token).map_err(|_| AuthError::TokenVerificationFailed)?;

        let kid = header
            .kid
            .as_ref()
            .ok_or(AuthError::TokenVerificationFailed)?;

        let jwk = jwks.find(kid).ok_or(AuthError::TokenVerificationFailed)?;

        let decoding_key = jsonwebtoken::DecodingKey::from_jwk(jwk)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        validation.set_audience(&[&self.client_id]);
        validation.set_required_spec_claims(&["sub", "email", "iss", "aud"]);

        let token_data = jsonwebtoken::decode::<GoogleClaims>(token, &decoding_key, &validation)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let claims = token_data.claims;

        validate_google_claims(claims)
    }
}

/// Validates decoded Google ID token claims and turns them into `GoogleUserInfo`,
/// separately from the JWKS-fetching/signature-verification mechanics above so this
/// business logic (in particular, the `email_verified` check) can be unit-tested
/// directly against hand-built claims, without needing a real signed token or network
/// access to Google's JWKS endpoint.
pub fn validate_google_claims(claims: GoogleClaims) -> Result<GoogleUserInfo, AuthError> {
    let email = claims.email.ok_or(AuthError::TokenVerificationFailed)?;

    // Google's guidance for "Sign in with Google" is to check email_verified before
    // trusting the email claim as an identifier. Without this, an account with an
    // unverified email could be used to create or match a user record for an email
    // address its holder doesn't actually control.
    if claims.email_verified != Some(true) {
        tracing::warn!("Google login rejected: email_verified is not true");
        return Err(AuthError::TokenVerificationFailed);
    }

    let name = claims.name.unwrap_or_else(|| email.clone());

    Ok(GoogleUserInfo { email, name })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GoogleClaims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_max_age_from_a_simple_cache_control_header() {
        assert_eq!(
            parse_max_age("max-age=21600"),
            Some(Duration::from_secs(21600))
        );
    }

    #[test]
    fn parses_max_age_among_other_directives() {
        assert_eq!(
            parse_max_age("public, max-age=3600, must-revalidate"),
            Some(Duration::from_secs(3600))
        );
    }

    #[test]
    fn parse_max_age_is_case_insensitive_on_the_directive_name() {
        assert_eq!(
            parse_max_age("Max-Age=60"),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn parse_max_age_returns_none_when_absent() {
        assert_eq!(parse_max_age("no-cache"), None);
    }

    #[test]
    fn parse_max_age_returns_none_on_malformed_value() {
        assert_eq!(parse_max_age("max-age=not-a-number"), None);
    }
}
