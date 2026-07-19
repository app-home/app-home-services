use serde::{Deserialize, Serialize};

const MAX_URL_LEN: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AvatarUrl(String);

impl AvatarUrl {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let s = value.into();
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            return Err("Avatar URL must not be empty".into());
        }
        if trimmed.len() > MAX_URL_LEN {
            return Err(format!(
                "Avatar URL must not exceed {MAX_URL_LEN} characters"
            ));
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}
