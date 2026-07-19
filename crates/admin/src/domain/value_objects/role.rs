use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "admin")]
    Admin,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Admin => "admin",
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

impl TryFrom<&str> for Role {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "user" => Ok(Role::User),
            "admin" => Ok(Role::Admin),
            other => Err(format!("Invalid role: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_user_as_str() {
        assert_eq!(Role::User.as_str(), "user");
    }

    #[test]
    fn role_admin_as_str() {
        assert_eq!(Role::Admin.as_str(), "admin");
    }

    #[test]
    fn role_user_is_not_admin() {
        assert!(!Role::User.is_admin());
    }

    #[test]
    fn role_admin_is_admin() {
        assert!(Role::Admin.is_admin());
    }

    #[test]
    fn role_try_from_valid() {
        assert_eq!(Role::try_from("user").unwrap(), Role::User);
        assert_eq!(Role::try_from("admin").unwrap(), Role::Admin);
    }

    #[test]
    fn role_try_from_invalid() {
        assert!(Role::try_from("superadmin").is_err());
    }

    #[test]
    fn role_serde_round_trip() {
        let json = serde_json::to_string(&Role::Admin).unwrap();
        assert_eq!(json, "\"admin\"");
        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Role::Admin);
    }
}
