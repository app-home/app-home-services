use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMethod {
    #[serde(rename = "password")]
    Password,
    #[serde(rename = "google_oauth")]
    GoogleOAuth,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::GoogleOAuth => "google_oauth",
        }
    }
}

impl TryFrom<&str> for AuthMethod {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "password" => Ok(Self::Password),
            "google_oauth" => Ok(Self::GoogleOAuth),
            other => Err(format!("Invalid auth method: {other}")),
        }
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
