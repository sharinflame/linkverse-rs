use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as, skip_serializing_none};

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Debug)]
pub struct FileContext {
    #[serde_as(as = "DisplayFromStr")]
    pub context_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    pub objects: Vec<String>,
    pub reference_count: i32,
    pub allowed_count: i32,
    pub created_at: i64,
    pub r#type: String,
}
