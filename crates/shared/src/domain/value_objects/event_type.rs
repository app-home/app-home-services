use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "login")]
    Login,
    #[serde(rename = "logout")]
    Logout,
    #[serde(rename = "refresh")]
    Refresh,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Logout => "logout",
            Self::Refresh => "refresh",
        }
    }
}

impl TryFrom<&str> for EventType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "login" => Ok(Self::Login),
            "logout" => Ok(Self::Logout),
            "refresh" => Ok(Self::Refresh),
            other => Err(format!("Invalid event type: {other}")),
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
