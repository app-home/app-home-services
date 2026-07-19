use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthProvider {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "google")]
    Google,
}

impl AuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Google => "google",
        }
    }
}

impl TryFrom<&str> for AuthProvider {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "local" => Ok(Self::Local),
            "google" => Ok(Self::Google),
            other => Err(format!("Invalid auth provider: {other}")),
        }
    }
}

impl fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
