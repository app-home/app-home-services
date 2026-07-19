use serde::{Deserialize, Serialize};

const MAX_BIO_LEN: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bio(String);

impl Bio {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let s = value.into();
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            return Err("Bio must not be empty".into());
        }
        if trimmed.len() > MAX_BIO_LEN {
            return Err(format!("Bio must not exceed {MAX_BIO_LEN} characters"));
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
