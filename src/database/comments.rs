use crate::{database::conn::LazyConn, entities::comment::Comment, utils::format::flatten_rows};
use deadpool_postgres::Transaction;
use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};
use validator::ValidateLength;

/// Creates a new comment in the database and returns the created Comment entity.
pub async fn create_comment(
    post_id: &i64,
    user_id: &i64,
    content: &String,
    parent_comment_id: Option<i64>,
    tx: &mut Transaction<'_>,
) -> Comment {
    let row = tx
        .query_one(
            "
            INSERT INTO comments (post_id, user_id, content, parent_comment_id)
            VALUES ($1, $2, $3, $4)
            RETURNING comment_id, post_id, user_id, content, parent_comment_id, likes_count, dislikes_count, replies_count, type
            ",
            // I had to put type cause it was giving an error with "expected &i64, found &String"
            &[&post_id, &user_id, &content, &parent_comment_id],
        )
        .await
        .unwrap();

    Comment {
        comment_id: row.get("comment_id"),
        post_id: row.get("post_id"),
        user_id: row.get("user_id"),
        content: row.get("content"),
        parent_commend_id: row.get("parent_comment_id"),
        likes_count: row.get("likes_count"),
        dislikes_count: row.get("dislikes_count"),
        replies_count: row.get("replies_count"),
        r#type: row.get("type"),
    }
}

/// Retrieves a comment by its ID from the database.
pub async fn get_comment(comment_id: &i64, conn: &mut LazyConn) -> Option<Comment> {
    let db = conn.get_client().await.unwrap();
    let row = db
        .query_opt(
            "
            SELECT comment_id, post_id, user_id, content, parent_comment_id, likes_count, dislikes_count, replies_count, type
            FROM comments
            WHERE comment_id = $1
            ",
            &[&comment_id],
        )
        .await
        .unwrap();

    row.map(|row| Comment {
        comment_id: row.get("comment_id"),
        post_id: row.get("post_id"),
        user_id: row.get("user_id"),
        content: row.get("content"),
        parent_commend_id: row.get("parent_comment_id"),
        likes_count: row.get("likes_count"),
        dislikes_count: row.get("dislikes_count"),
        replies_count: row.get("replies_count"),
        r#type: row.get("type"),
    })
}

/// Soft deletes a comment by setting its user_id and content to NULL.
pub async fn soft_delete_comment(comment_id: &i64, tx: &mut Transaction<'_>) -> bool {
    let result = tx
        .execute(
            "
            UPDATE comments
            SET user_id = NULL, content = NULL
            WHERE comment_id = $1
            ",
            &[&comment_id],
        )
        .await
        .unwrap();

    result == 1
}

/// Permanently deletes a comment from the database.
pub async fn delete_comment(comment_id: &i64, tx: &mut Transaction<'_>) -> bool {
    let result = tx
        .execute(
            "
            DELETE FROM comments
            WHERE comment_id = $1
            ",
            &[&comment_id],
        )
        .await
        .unwrap();

    result == 1
}

fn parse_cursor(s: &str) -> Option<(i64, i64, i64)> {
    let mut parts = s.split(',').map(|x| x.trim().parse::<i64>());
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

#[serde_as]
#[derive(Serialize)]
pub struct CommentsList {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub comments: Vec<i64>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

pub async fn get_comments(
    post_id: &i64,
    cursor: Option<String>,
    user_id: &i64,
    conn: &mut LazyConn,
    r#type: Option<String>,
    parent_id: Option<i64>,
    limit: i64,
) -> Result<CommentsList, &'static str> {
    let db = conn.get_client().await.unwrap();

    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![post_id, user_id];

    let mut sql = "
        WITH ranked_comments AS (
            SELECT comment_id,
                   popularity_score,
                   CASE WHEN user_id = $2 THEN 1 ELSE 0 END AS is_user_comment
            FROM comments
            WHERE post_id = $1
        )

        SELECT * FROM ranked_comments
    "
    .to_string();

    let is_user: i64;
    let popularity_score: i64;
    let comment_id: i64;
    let comment_parent_id: i64;
    let comment_type: String;

    if let Some(cursor) = cursor {
        let (i_u, ps, ci) = parse_cursor(&cursor).ok_or("INCORRECT_CURSOR")?;
        is_user = i_u;
        popularity_score = ps;
        comment_id = ci;

        sql += "
            WHERE (is_user_comment < $3 OR
                   (is_user_comment = $3 AND popularity_score < $4) OR
                   (is_user_comment = $3 AND popularity_score = $4
                    AND comment_id < $5))
        ";
        params.push(&is_user);
        params.push(&popularity_score);
        params.push(&comment_id);

        if let Some(parent_id) = parent_id {
            comment_parent_id = parent_id;
            sql += " AND parent_comment_id = $6";
            params.push(&comment_parent_id);
        } else {
            sql += " AND parent_comment_id IS NULL";
        }
    } else {
        if let Some(parent_id) = parent_id {
            comment_parent_id = parent_id;
            sql += " WHERE parent_comment_id = $3";
            params.push(&comment_parent_id);
        } else {
            sql += " WHERE parent_comment_id IS NULL";
        }
    }

    if let Some(ref r#type) = r#type {
        comment_type = r#type.clone();
        sql += &format!(
            "
            AND type = ${}
        ",
            params.length().unwrap_or(0) + 1
        );
        params.push(&comment_type);
    }

    if r#type.is_none() || r#type == Some("comment".to_string()) {
        sql += "
            ORDER BY is_user_comment DESC, popularity_score DESC,
                    comment_id::bigint DESC
        "
    } else if r#type == Some("update".to_string()) {
        sql += "
            ORDER BY comment_id::bigint
        "
    }

    sql += &format!("LIMIT {}", &(params.length().unwrap() + 1));
    let temp_limit = limit + 1;
    params.push(&temp_limit);

    let mut rows = db.query(&sql, &params).await.unwrap();
    let has_more = rows.len() > limit as usize;

    if rows.len() > 0 {
        rows.remove(rows.len() - 1);
    }

    let mut next_cursor: Option<String> = None;
    if let Some(last_row) = rows.last() {
        let is_user: i64 = last_row.get("is_user_comment");
        let popularity: i64 = last_row.get("popularity_score");
        let comment_id: i64 = last_row.get("comment_id");

        next_cursor = Some(format!("{},{},{}", is_user, popularity, comment_id));
    }

    let comments: Vec<i64> = flatten_rows(rows, "comment_id");
    return Ok(CommentsList {
        comments,
        next_cursor,
        has_more,
    });
}
