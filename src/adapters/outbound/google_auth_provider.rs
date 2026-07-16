use std::future::Future;
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

/// A time-based cache for a single JWKS value, deliberately decoupled from *how* the
/// value is fetched (the `fetch` closure passed to `get_or_fetch`). Kept separate
/// from `GoogleAuthProvider` so the cache hit/miss/expiry/stale-fallback behavior can
/// be unit-tested against a fake fetch function, without any network access or a
/// real Google response.
#[derive(Debug, Clone)]
struct JwksCache {
    inner: Arc<RwLock<Option<CachedJwks>>>,
}

impl JwksCache {
    fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the cached JWKS if still fresh; otherwise calls `fetch` to obtain a
    /// new one (plus its TTL) and caches it.
    ///
    /// If `fetch` fails and a value is still cached from before (even if expired),
    /// serves that stale value instead of propagating the error -- a transient
    /// upstream failure shouldn't fail every call while we still hold keys that are
    /// very likely still valid. The error only propagates when there's nothing
    /// cached to fall back on.
    async fn get_or_fetch<F, Fut>(&self, fetch: F) -> Result<jsonwebtoken::jwk::JwkSet, AuthError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(jsonwebtoken::jwk::JwkSet, Duration), AuthError>>,
    {
        if let Some(cached) = self.inner.read().await.as_ref()
            && cached.expires_at > Instant::now()
        {
            return Ok(cached.jwks.clone());
        }

        match fetch().await {
            Ok((jwks, ttl)) => {
                let cached = CachedJwks {
                    jwks: jwks.clone(),
                    expires_at: Instant::now() + ttl,
                };
                *self.inner.write().await = Some(cached);
                Ok(jwks)
            }
            Err(e) => {
                if let Some(stale) = self.inner.read().await.as_ref() {
                    tracing::warn!(
                        error = %e,
                        "Failed to refresh JWKS; serving stale cached keys"
                    );
                    return Ok(stale.jwks.clone());
                }
                Err(e)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoogleAuthProvider {
    client_id: String,
    /// A single shared client, built once in `new`, so TCP connections/TLS sessions
    /// to Google's certs endpoint get pooled and reused across requests instead of
    /// paying fresh connection setup on every login.
    http_client: reqwest::Client,
    jwks_cache: JwksCache,
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
            jwks_cache: JwksCache::new(),
        }
    }

    async fn get_jwks(&self) -> Result<jsonwebtoken::jwk::JwkSet, AuthError> {
        let client = self.http_client.clone();
        self.jwks_cache
            .get_or_fetch(|| async move { fetch_jwks_from_google(&client).await })
            .await
    }
}

async fn fetch_jwks_from_google(
    client: &reqwest::Client,
) -> Result<(jsonwebtoken::jwk::JwkSet, Duration), AuthError> {
    let response = client
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

    Ok((jwks, ttl))
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
mod max_age_tests {
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
        assert_eq!(parse_max_age("Max-Age=60"), Some(Duration::from_secs(60)));
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

#[cfg(test)]
mod jwks_cache_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn empty_jwks() -> jsonwebtoken::jwk::JwkSet {
        jsonwebtoken::jwk::JwkSet { keys: Vec::new() }
    }

    #[tokio::test]
    async fn first_call_is_a_cache_miss_and_invokes_fetch() {
        let cache = JwksCache::new();
        let calls = AtomicUsize::new(0);

        let result = cache
            .get_or_fetch(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok((empty_jwks(), Duration::from_secs(60)))
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn second_call_within_ttl_is_a_cache_hit_and_does_not_invoke_fetch() {
        let cache = JwksCache::new();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_fetch(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok((empty_jwks(), Duration::from_secs(60)))
            })
            .await
            .unwrap();

        cache
            .get_or_fetch(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok((empty_jwks(), Duration::from_secs(60)))
            })
            .await
            .unwrap();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second call inside the TTL should be served from cache without re-fetching"
        );
    }

    #[tokio::test]
    async fn call_after_ttl_expiry_re_invokes_fetch() {
        let cache = JwksCache::new();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_fetch(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                // Deliberately tiny TTL so the test doesn't need to wait long.
                Ok((empty_jwks(), Duration::from_millis(20)))
            })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        cache
            .get_or_fetch(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok((empty_jwks(), Duration::from_secs(60)))
            })
            .await
            .unwrap();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "a call after the cached TTL has expired should re-fetch"
        );
    }

    #[tokio::test]
    async fn fetch_failure_with_a_stale_cache_serves_the_stale_value() {
        let cache = JwksCache::new();

        cache
            .get_or_fetch(|| async { Ok((empty_jwks(), Duration::from_millis(10))) })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(30)).await;

        let result = cache
            .get_or_fetch(|| async {
                Err::<(jsonwebtoken::jwk::JwkSet, Duration), AuthError>(
                    AuthError::TokenVerificationFailed,
                )
            })
            .await;

        assert!(
            result.is_ok(),
            "a fetch failure should fall back to the stale cached value, got {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_failure_with_no_cache_propagates_the_error() {
        let cache = JwksCache::new();

        let result = cache
            .get_or_fetch(|| async {
                Err::<(jsonwebtoken::jwk::JwkSet, Duration), AuthError>(
                    AuthError::TokenVerificationFailed,
                )
            })
            .await;

        assert!(
            result.is_err(),
            "a fetch failure with nothing cached should propagate the error"
        );
    }
}
