use crate::utils::snowflake::SnowflakeGenerator;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthUser {
    pub username: String,
    pub user_id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub email_verified: Option<bool>,
    pub pending_email: Option<String>,
    pub pending_email_until: Option<i64>,
}

/// Struct for giving to frontend
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub user_id: String,
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
