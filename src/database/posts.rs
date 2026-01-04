use tokio_postgres::Row;

use crate::{
    database::conn::LazyConn,
    entities::post::Post,
    utils::{state::ArcAppState, storage::build_links},
};

pub static POST_SQL: &str = "
    SELECT p.post_id, p.user_id, p.content,
           EXTRACT(EPOCH FROM p.created_at),
           EXTRACT(EPOCH FROM p.updated_at),
           p.likes_count, p.comments_count,
           p.dislikes_count, p.tags, m.objects as media,
           m.type as media_type
           COALESCE(
                array_agg(t.name)
                FILTER (WHERE t.tag_id IS NOT NULL),
                '{{}}'
           ) AS ctags
    FROM posts p
    LEFT JOIN post_tags pt ON pt.post_id = p.post_id
    LEFT JOIN tags t ON t.tag_id = pt.tag_id
    LEFT JOIN files m ON m.context_id = p.file_context_id

    WHERE p.post_id = $1 AND ($2::bool OR p.is_deleted = false)

    GROUP BY p.post_id, m.objects, m.type
";

/// Private function to get Post entity from Row
/// Row needs to have all the non-option fields of Post
fn row_to_post(row: Row, state: &ArcAppState) -> Post {
    Post {
        post_id: row.get("post_id"),
        user_id: row.get("user_id"),
        content: row.get("content"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        likes_count: row.get("likes_count"),
        dislikes_count: row.get("dislikes_count"),
        comments_count: row.get("comments_count"),
        flags: row.get("flags"),
        media: build_links(row.get("media"), state),
        media_type: row.get("media_type"),
        status: row.get("status"),
        is_deleted: row.get("is_deleted"),
        tags: row.get("tags"),
    }
}

/// Get single post by id from database
/// Returns: Post entity without 'is_deleted' and 'status' fields
pub async fn get_post(
    post_id: &String,
    conn: &mut LazyConn,
    state: &ArcAppState,
    include_deleted: bool,
) -> Option<Post> {
    let db = conn.get_client().await.unwrap();
    let row = db
        .query_opt(POST_SQL, &[&post_id, &include_deleted])
        .await
        .unwrap();
    row.map(|r| row_to_post(r, state))
}
