use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use app_home_services::profiles::adapters::inbound::responses::{
    ProfileResponse, UpdateProfileRequest,
};
use app_home_services::profiles::application::ports::profile_repository::ProfileRepository;
use app_home_services::profiles::application::use_cases::{get_profile, update_profile};
use app_home_services::profiles::domain::entities::profile::UserProfile;
use app_home_services::profiles::domain::errors::ProfileError;
use app_home_services::profiles::domain::value_objects::avatar_url::AvatarUrl;
use app_home_services::profiles::domain::value_objects::bio::Bio;

// ---------------------------------------------------------------------------
// Mock repository
// ---------------------------------------------------------------------------

struct MockProfileRepo {
    store: Mutex<HashMap<Uuid, UserProfile>>,
}

impl MockProfileRepo {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    fn seed(&self, user_id: Uuid, avatar_url: Option<AvatarUrl>, bio: Option<Bio>) {
        let profile = UserProfile::new(user_id, avatar_url, bio, Utc::now());
        self.store.lock().unwrap().insert(user_id, profile);
    }
}

#[async_trait]
impl ProfileRepository for MockProfileRepo {
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<UserProfile>, ProfileError> {
        Ok(self.store.lock().unwrap().get(&user_id).cloned())
    }

    async fn upsert(&self, profile: &UserProfile) -> Result<(), ProfileError> {
        self.store
            .lock()
            .unwrap()
            .insert(profile.user_id(), profile.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AvatarUrl
// ---------------------------------------------------------------------------

#[test]
fn avatar_url_valid_url() {
    let url = AvatarUrl::new("https://example.com/avatar.png").unwrap();
    assert_eq!(url.as_str(), "https://example.com/avatar.png");
}

#[test]
fn avatar_url_trims_whitespace() {
    let url = AvatarUrl::new("  https://example.com/avatar.png  ").unwrap();
    assert_eq!(url.as_str(), "https://example.com/avatar.png");
}

#[test]
fn avatar_url_rejects_empty() {
    let err = AvatarUrl::new("").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn avatar_url_rejects_whitespace_only() {
    let err = AvatarUrl::new("   ").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn avatar_url_rejects_too_long() {
    let long = "https://a.com/".to_string() + &"x".repeat(490);
    assert!(long.len() > 500);
    let err = AvatarUrl::new(long).unwrap_err();
    assert!(err.contains("500"));
}

#[test]
fn avatar_url_accepts_max_length() {
    let prefix = "https://a.com/";
    let padding_len = 500 - prefix.len();
    let long = prefix.to_string() + &"x".repeat(padding_len);
    assert_eq!(long.len(), 500);
    AvatarUrl::new(long).unwrap();
}

#[test]
fn avatar_url_into_inner() {
    let url = AvatarUrl::new("https://example.com/avatar.png").unwrap();
    assert_eq!(url.into_inner(), "https://example.com/avatar.png");
}

#[test]
fn avatar_url_partial_eq() {
    let a = AvatarUrl::new("https://example.com/a.png").unwrap();
    let b = AvatarUrl::new("https://example.com/a.png").unwrap();
    let c = AvatarUrl::new("https://example.com/c.png").unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------------
// Bio
// ---------------------------------------------------------------------------

#[test]
fn bio_valid_text() {
    let bio = Bio::new("Hello, world!").unwrap();
    assert_eq!(bio.as_str(), "Hello, world!");
}

#[test]
fn bio_trims_whitespace() {
    let bio = Bio::new("  Hello, world!  ").unwrap();
    assert_eq!(bio.as_str(), "Hello, world!");
}

#[test]
fn bio_rejects_empty() {
    let err = Bio::new("").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn bio_rejects_whitespace_only() {
    let err = Bio::new("   ").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn bio_rejects_too_long() {
    let long = "x".repeat(2001);
    let err = Bio::new(long).unwrap_err();
    assert!(err.contains("2000"));
}

#[test]
fn bio_accepts_max_length() {
    let long = "x".repeat(2000);
    Bio::new(long).unwrap();
}

#[test]
fn bio_into_inner() {
    let bio = Bio::new("Short bio").unwrap();
    assert_eq!(bio.into_inner(), "Short bio");
}

#[test]
fn bio_partial_eq() {
    let a = Bio::new("Bio A").unwrap();
    let b = Bio::new("Bio A").unwrap();
    let c = Bio::new("Bio C").unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------------
// UserProfile
// ---------------------------------------------------------------------------

#[test]
fn user_profile_creation_with_all_fields() {
    let user_id = Uuid::now_v7();
    let avatar = AvatarUrl::new("https://example.com/avatar.png").ok();
    let bio = Bio::new("A short bio").ok();
    let now = Utc::now();

    let profile = UserProfile::new(user_id, avatar.clone(), bio.clone(), now);

    assert_eq!(profile.user_id(), user_id);
    assert_eq!(profile.avatar_url(), avatar.as_ref());
    assert_eq!(profile.bio(), bio.as_ref());
    assert_eq!(profile.updated_at(), now);
}

#[test]
fn user_profile_creation_without_optionals() {
    let user_id = Uuid::now_v7();
    let now = Utc::now();
    let profile = UserProfile::new(user_id, None, None, now);

    assert_eq!(profile.user_id(), user_id);
    assert!(profile.avatar_url().is_none());
    assert!(profile.bio().is_none());
}

#[test]
fn user_profile_creation_with_only_avatar() {
    let user_id = Uuid::now_v7();
    let avatar = AvatarUrl::new("https://example.com/avatar.png").ok();
    let now = Utc::now();
    let profile = UserProfile::new(user_id, avatar.clone(), None, now);

    assert_eq!(profile.avatar_url(), avatar.as_ref());
    assert!(profile.bio().is_none());
}

// ---------------------------------------------------------------------------
// ProfileError
// ---------------------------------------------------------------------------

#[test]
fn profile_error_not_found_message() {
    let user_id = Uuid::now_v7();
    let err = ProfileError::NotFound(user_id);
    let msg = err.to_string();
    assert!(msg.contains(&user_id.to_string()));
    assert!(msg.contains("not found"));
}

#[test]
fn profile_error_invalid_value_message() {
    let err = ProfileError::InvalidValue("bad url".into());
    assert_eq!(err.to_string(), "Invalid value: bad url");
}

#[test]
fn profile_error_internal_error_message() {
    let err = ProfileError::InternalError("db failure".into());
    assert_eq!(err.to_string(), "Internal error: db failure");
}

// ---------------------------------------------------------------------------
// get_profile use case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_profile_found() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();
    let avatar = AvatarUrl::new("https://example.com/avatar.png").unwrap();
    let bio = Bio::new("Hello").unwrap();
    repo.seed(user_id, Some(avatar.clone()), Some(bio.clone()));

    let result = get_profile::get_profile(&repo, user_id).await.unwrap();
    assert_eq!(result.user_id(), user_id);
    assert_eq!(result.avatar_url(), Some(&avatar));
    assert_eq!(result.bio(), Some(&bio));
}

#[tokio::test]
async fn get_profile_not_found() {
    let repo = MockProfileRepo::new();
    let missing_id = Uuid::now_v7();

    let err = get_profile::get_profile(&repo, missing_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ProfileError::NotFound(id) if id == missing_id));
}

// ---------------------------------------------------------------------------
// update_profile use case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_profile_creates_new_profile() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();

    let result = update_profile::update_profile(
        &repo,
        user_id,
        Some("https://example.com/avatar.png".into()),
        Some("My bio".into()),
    )
    .await
    .unwrap();

    assert_eq!(result.user_id(), user_id);
    assert_eq!(
        result.avatar_url().map(|a| a.as_str()),
        Some("https://example.com/avatar.png")
    );
    assert_eq!(result.bio().map(|b| b.as_str()), Some("My bio"));
}

#[tokio::test]
async fn update_profile_updates_existing() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();
    repo.seed(
        user_id,
        Some(AvatarUrl::new("https://old.com/avatar.png").unwrap()),
        Some(Bio::new("Old bio").unwrap()),
    );

    let result = update_profile::update_profile(
        &repo,
        user_id,
        Some("https://new.com/avatar.png".into()),
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        result.avatar_url().map(|a| a.as_str()),
        Some("https://new.com/avatar.png")
    );
    assert_eq!(result.bio().map(|b| b.as_str()), Some("Old bio"));
}

#[tokio::test]
async fn update_profile_preserves_existing_on_none() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();
    repo.seed(
        user_id,
        Some(AvatarUrl::new("https://old.com/avatar.png").unwrap()),
        Some(Bio::new("Old bio").unwrap()),
    );

    let result = update_profile::update_profile(&repo, user_id, None, None)
        .await
        .unwrap_err();
    assert!(matches!(result, ProfileError::InvalidValue(_)));
}

#[tokio::test]
async fn update_profile_clears_field_with_empty_string() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();
    repo.seed(
        user_id,
        Some(AvatarUrl::new("https://old.com/avatar.png").unwrap()),
        Some(Bio::new("Old bio").unwrap()),
    );

    let result = update_profile::update_profile(&repo, user_id, None, Some("".into()))
        .await
        .unwrap();

    assert_eq!(
        result.avatar_url().map(|a| a.as_str()),
        Some("https://old.com/avatar.png")
    );
    assert!(result.bio().is_none());
}

#[tokio::test]
async fn update_profile_rejects_invalid_avatar_url() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();

    let long_url = "https://a.com/".to_string() + &"x".repeat(500);
    let err = update_profile::update_profile(&repo, user_id, Some(long_url), None)
        .await
        .unwrap_err();
    assert!(matches!(err, ProfileError::InvalidValue(_)));
    assert!(err.to_string().contains("avatar_url"));
}

#[tokio::test]
async fn update_profile_rejects_invalid_bio() {
    let repo = MockProfileRepo::new();
    let user_id = Uuid::now_v7();

    let err = update_profile::update_profile(&repo, user_id, None, Some("x".repeat(2001)))
        .await
        .unwrap_err();
    assert!(matches!(err, ProfileError::InvalidValue(_)));
    assert!(err.to_string().contains("bio"));
}

// ---------------------------------------------------------------------------
// Serialization / response types
// ---------------------------------------------------------------------------

#[test]
fn profile_response_round_trip() {
    let resp = ProfileResponse {
        user_id: Uuid::now_v7().to_string(),
        avatar_url: Some("https://example.com/avatar.png".into()),
        bio: Some("Hello".into()),
        updated_at: "2026-07-19T12:00:00Z".into(),
    };
    let json = serde_json::to_value(&resp).unwrap();
    let deserialized: ProfileResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp.user_id, deserialized.user_id);
    assert_eq!(resp.avatar_url, deserialized.avatar_url);
    assert_eq!(resp.bio, deserialized.bio);
}

#[test]
fn profile_response_nullable_fields() {
    let resp = ProfileResponse {
        user_id: Uuid::now_v7().to_string(),
        avatar_url: None,
        bio: None,
        updated_at: "2026-07-19T12:00:00Z".into(),
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json.get("avatar_url").is_some());
    assert!(json["avatar_url"].is_null());
    let deserialized: ProfileResponse = serde_json::from_value(json).unwrap();
    assert!(deserialized.avatar_url.is_none());
}

#[test]
fn update_profile_request_round_trip() {
    let req = UpdateProfileRequest {
        avatar_url: Some("https://example.com/avatar.png".into()),
        bio: Some("New bio".into()),
    };
    let json = serde_json::to_value(&req).unwrap();
    let deserialized: UpdateProfileRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.avatar_url, deserialized.avatar_url);
    assert_eq!(req.bio, deserialized.bio);
}

#[test]
fn update_profile_request_all_null() {
    let req = UpdateProfileRequest {
        avatar_url: None,
        bio: None,
    };
    let json = serde_json::to_value(&req).unwrap();
    let deserialized: UpdateProfileRequest = serde_json::from_value(json).unwrap();
    assert!(deserialized.avatar_url.is_none());
    assert!(deserialized.bio.is_none());
}
