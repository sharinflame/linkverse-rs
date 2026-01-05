use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as, skip_serializing_none};

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthUser {
    pub username: String,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub email_verified: Option<bool>,
    pub pending_email: Option<String>,
    pub pending_email_until: Option<i64>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub role_id: i32,
    pub following_count: Option<i64>,
    pub followers_count: Option<i64>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub bio: Option<String>,
    pub badges: Option<Vec<i16>>,
    pub languages: Option<Vec<String>>,
}
