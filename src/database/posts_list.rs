use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};
use tokio_postgres::{Row, types::FromSqlOwned};

use crate::database::conn::LazyConn;

#[serde_as]
#[derive(Serialize)]
pub struct PostsList {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub posts: Vec<i64>,
    pub next_cursor: Option<String>,
}

fn parse_tuple(s: &str) -> Option<(i64, i64)> {
    let mut parts = s.split(',').map(|x| x.trim().parse::<i64>());
    Some((parts.next()?.ok()?, parts.next()?.ok()?))
}

#[inline]
fn flatten_rows<T>(from: Vec<Row>, key: &str) -> Vec<T>
where
    T: FromSqlOwned,
{
    let mut to: Vec<T> = Vec::new();

    for row in from {
        to.push(row.get(key));
    }

    to
}

pub async fn get_popular_posts(
    user_id: &i64,
    conn: &mut LazyConn,
    limit: i64,
    cursor: Option<String>,
    hide_viewed: bool,
) -> Result<PostsList, &'static str> {
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![&limit, user_id];
    let popularity_score: i64;
    let post_id: i64;

    let mut sql = "
        SELECT post_id, popularity_score
        FROM posts
        WHERE is_deleted = FALSE AND user_id != $2 
    "
    .to_string();

    if hide_viewed {
        sql += "
            AND NOT EXISTS (
                SELECT 1
                FROM user_post_views
                WHERE user_post_views.user_id = $2
                AND user_post_views.post_id = posts.post_id
            ) 
        "
    }

    if let Some(cursor) = cursor {
        (popularity_score, post_id) = parse_tuple(&cursor).ok_or("INCORRECT_CURSOR")?;
        sql += "
            AND (
                (popularity_score) < $3 OR
                ((popularity_score) = $3 AND post_id < $4)
            ) 
        ";
        params.push(&popularity_score);
        params.push(&post_id);
    }

    sql += "
        ORDER BY popularity_score DESC, post_id DESC
        LIMIT $1
    ";

    let client = conn.get_client().await.unwrap();

    let rows = client.query(&sql, &params).await.unwrap();

    let last_row = rows.last();
    let next_cursor: Option<String> = match last_row {
        Some(r) => {
            let popularity: i64 = r.get("popularity_score");
            let post_id: i64 = r.get("post_id");
            Some(format!("{},{}", popularity, post_id))
        }
        None => None,
    };

    let posts: Vec<i64> = flatten_rows(rows, "post_id");
    Ok(PostsList { posts, next_cursor })
}
