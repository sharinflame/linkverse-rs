use serde::Serialize;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize, Debug)]
pub struct Post {
    pub post_id: String,
    pub user_id: String,
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
