pub mod api_doc;
pub mod health;
/// Builds the application router outside `main`, so the app can be constructed
/// and exercised without starting the process (see #191).
pub mod router;
/// Applies the HTTP security headers emitted on every response (see #90) and
/// provides the automated native-TLS smoke test backing issue #93.
pub mod security_headers;

pub use admin;
pub use auth::{AppState, application, domain};
pub use infrastructure;
pub use profiles;
pub use shared;
