use std::collections::HashSet;

use deadpool_postgres::Transaction;
use tokio_postgres::Row;

use crate::{
    database::conn::LazyConn,
    entities::post::Post,
    utils::{format::normalize_tag, storage::build_links, thread_state::generate_id},
};

pub static POST_SQL: &str = "
    SELECT p.post_id, p.user_id, p.content,
           EXTRACT(EPOCH FROM p.created_at)::bigint as created_at,
           EXTRACT(EPOCH FROM p.updated_at)::bigint as updated_at,
           p.likes_count, p.comments_count,
           p.dislikes_count, p.flags,
           COALESCE(m.objects, '{}'::text[]) AS media,
           m.type as media_type,
           COALESCE(
                array_agg(t.name)
                FILTER (WHERE t.tag_id IS NOT NULL),
                '{{}}'
           ) AS tags
    FROM posts p
    LEFT JOIN post_tags pt ON pt.post_id = p.post_id
    LEFT JOIN tags t ON t.tag_id = pt.tag_id
    LEFT JOIN files m ON m.context_id = p.file_context_id

    WHERE p.post_id = $1 AND ($2::bool OR p.is_deleted = false)

    GROUP BY p.post_id, m.objects, m.type
";

/// Private function to get Post entity from Row
/// Row needs to have all the non-option fields of Post
fn row_to_post(row: Row) -> Post {
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
        media: build_links(row.get("media")),
        media_type: row.get("media_type"),
        status: row.try_get("status").ok().flatten(),
        is_deleted: row.try_get("is_deleted").ok().flatten(),
        tags: row.get("tags"),
    }
}

/// Get single post by id from database
/// Returns: Post entity without 'is_deleted' and 'status' fields
pub async fn get_post(post_id: &i64, conn: &mut LazyConn, include_deleted: bool) -> Option<Post> {
    let db = conn.get_client().await.unwrap();
    let row = db
        .query_opt(POST_SQL, &[&post_id, &include_deleted])
        .await
        .unwrap();
    row.map(row_to_post)
}

/// Functions that automatically creates tags and inserts them to post
/// Tags are normalized and duplicates are deleted before tags are inserted
/// Doesn't return anything, touches two tables "tags" and "post_tags" in database
pub async fn insert_tags_and_link_post(
    post_id: &i64,
    raw_tags: Vec<String>,
    tx: &mut Transaction<'_>,
) {
    let mut seen = HashSet::new();
    let mut names: Vec<String> = Vec::new();

    for t in raw_tags.into_iter() {
        let nt = normalize_tag(&t);
        if nt.is_empty() {
            continue;
        }
        if seen.insert(nt.clone()) {
            names.push(nt);
        }
    }
    drop(seen); // I'm just paranoid :3

    if names.is_empty() {
        return;
    }

    let ids: Vec<i64> = (0..names.len()).map(|_| generate_id()).collect();

    let name_slices: Vec<&str> = names.iter().map(|s| s.as_str()).collect();

    tx.execute(
        "
        INSERT INTO tags (name, tag_id)
        SELECT u.name, u.id
        FROM unnest($1::text[], $2::bigint[]) AS u(name, id)
        ON CONFLICT (name) DO NOTHING
        ",
        &[&name_slices, &ids],
    )
    .await
    .unwrap();

    tx.execute(
        "
        INSERT INTO post_tags (post_id, tag_id)
        SELECT $1, t.tag_id
        FROM tags t
        WHERE t.name = ANY($2::text[])
        ON CONFLICT DO NOTHING
        ",
        &[&post_id, &name_slices],
    )
    .await
    .unwrap();
}

// Function to create post, if tags are Some function 'insert_tags_and_link_post' is used
// Returns post_id so for getting post entity use 'get_post' after creating
pub async fn create_post(
    user_id: &i64,
    content: &String,
    flags: &Vec<String>,
    tags: Option<Vec<String>>, // Easier when it's not a reference
    file_context_id: &Option<i64>,
    tx: &mut Transaction<'_>,
) -> i64 {
    let post_id = generate_id();
    tx.execute(
        "
        INSERT INTO posts
        (post_id, user_id, content, flags, file_context_id)

        VALUES ($1, $2, $3, $4, $5)
        ",
        &[&post_id, user_id, content, flags, file_context_id],
    )
    .await
    .unwrap();

    if let Some(tags) = tags {
        insert_tags_and_link_post(&post_id, tags, tx).await;
    }
    post_id
}
