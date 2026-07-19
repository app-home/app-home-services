use serde::{Deserialize, Serialize};

use super::{access_token::AccessToken, refresh_token::RefreshToken};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
}

impl TokenPair {
    pub fn new(access_token: AccessToken, refresh_token: RefreshToken) -> Self {
        Self {
            access_token,
            refresh_token,
        }
    }

    pub fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    pub fn refresh_token(&self) -> &RefreshToken {
        &self.refresh_token
    }
}
