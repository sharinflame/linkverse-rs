use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as, skip_serializing_none};

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Debug)]
pub struct Post {
    #[serde_as(as = "DisplayFromStr")]
    pub post_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub likes_count: i64,
    pub dislikes_count: i64,
    pub comments_count: i64,
    pub flags: Vec<String>,
    pub media: Vec<String>,
    pub media_type: Option<String>,
    pub status: Option<String>,
    pub is_deleted: Option<bool>,
    pub tags: Option<Vec<String>>,
}
