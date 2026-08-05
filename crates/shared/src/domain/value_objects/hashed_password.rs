use std::fmt;

use serde::{Deserialize, Serialize};

/// Redaction marker used instead of the hash in every `Display`/`Debug` output,
/// so a hash (and its salt) can never reach logs accidentally. Matches the
/// `<redacted>` marker used for other secrets in the config Debug impls
/// (`shared::config::settings`, `auth::config::auth_settings`); use
/// `as_str()`/`into_inner()` to access the raw hash deliberately.
pub const REDACTED_HASH_MARKER: &str = "<redacted>";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashedPassword(String);

impl HashedPassword {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let s = value.into();
        if s.is_empty() {
            return Err("Password hash must not be empty".to_string());
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for HashedPassword {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HashedPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{REDACTED_HASH_MARKER}")
    }
}

impl fmt::Debug for HashedPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HashedPassword(\"{REDACTED_HASH_MARKER}\")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_redacts_the_hash() {
        let hash = HashedPassword::new("$2b$12$some-hash-with-salt").unwrap();
        assert_eq!(format!("{hash}"), "<redacted>");
        assert_eq!(hash.to_string(), "<redacted>");
    }

    #[test]
    fn debug_redacts_the_hash() {
        let hash = HashedPassword::new("$2b$12$some-hash-with-salt").unwrap();
        let rendered = format!("{hash:?}");
        assert!(
            !rendered.contains("some-hash-with-salt"),
            "Debug must not contain the hash, got: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn the_raw_hash_is_only_available_via_explicit_accessors() {
        let hash = HashedPassword::new("$2b$12$some-hash-with-salt").unwrap();
        assert_eq!(hash.as_str(), "$2b$12$some-hash-with-salt");
    }
}
