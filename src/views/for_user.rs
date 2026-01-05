use serde::Serialize;
use serde_with::skip_serializing_none;

use crate::{
    database::{conn::LazyConn, posts::get_post, users::get_min_user},
    entities::{post::Post, user::User},
};

/// Completed post that is shown to user
#[skip_serializing_none]
#[derive(Serialize, Debug)]
pub struct ForUserPostView {
    #[serde(flatten)]
    pub post: Post,
    pub user: Option<User>,
    pub is_fav: Option<bool>,
    pub is_like: Option<bool>,
}

pub async fn get_full_post(
    post: Post,
    for_user_id: &i64,
    conn: &mut LazyConn,
) -> ForUserPostView {
    let user = get_min_user(&post.user_id, conn).await;
    let (is_fav, is_like) = (None, None); // TODO: fill with actual reactions

    ForUserPostView {
        post,
        user,
        is_fav,
        is_like,
    }
}

pub async fn get_full_post_by_id(
    post_id: &i64,
    for_user_id: &i64,
    conn: &mut LazyConn,
    include_deleted: bool,
) -> Option<ForUserPostView> {
    let post = get_post(post_id, conn, include_deleted).await?;

    Some(get_full_post(post, for_user_id, conn).await)
}
