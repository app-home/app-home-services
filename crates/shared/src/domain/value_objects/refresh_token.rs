use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshToken(String);

impl RefreshToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for RefreshToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefreshToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for RefreshToken {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<RefreshToken> for String {
    fn from(token: RefreshToken) -> Self {
        token.0
    }
}
