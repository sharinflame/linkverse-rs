use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as, skip_serializing_none};

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Debug)]
pub struct Comment {
    #[serde_as(as = "DisplayFromStr")]
    pub comment_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub post_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    pub content: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub parent_commend_id: Option<i64>,
    pub likes_count: i64,
    pub dislikes_count: i64,
    pub replies_count: i64,
    pub r#type: String,
}
