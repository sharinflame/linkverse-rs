use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};

use crate::{database::conn::LazyConn, utils::format::flatten_rows};

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

pub enum PostsMode {
    Popular,
    New,
    ByFollowing,
}

pub async fn get_posts(
    mode: PostsMode,
    user_id: &i64,
    conn: &mut LazyConn,
    limit: i64,
    cursor: Option<String>,
    hide_viewed: bool,
) -> Result<PostsList, &'static str> {
    // params: $1 = limit, $2 = user_id, optionally $3/$4 for cursor
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![&limit, user_id];

    let mut sql = match mode {
        PostsMode::Popular => "
            SELECT post_id, popularity_score
            FROM posts
            WHERE is_deleted = FALSE AND user_id != $2
            "
        .to_string(),
        PostsMode::New => "
            SELECT post_id
            FROM posts
            WHERE is_deleted = FALSE AND user_id != $2
            "
        .to_string(),
        PostsMode::ByFollowing => "
            SELECT post_id
            FROM posts
            WHERE is_deleted = FALSE AND EXISTS (
                SELECT 1
                FROM followed
                WHERE followed.user_id = $2
                AND followed.followed_to = posts.user_id
            )
            "
        .to_string(),
    };

    if hide_viewed {
        sql += "
            AND NOT EXISTS (
                SELECT 1
                FROM user_post_views
                WHERE user_post_views.user_id = $2
                AND user_post_views.post_id = posts.post_id
            )
        ";
    }

    // keep parsed cursor values in locals so references we push live until query
    let parsed_popularity: i64;
    let parsed_post_id: i64;

    match (&mode, cursor) {
        (PostsMode::Popular, Some(c)) => {
            let (p, pid) = parse_tuple(&c).ok_or("INCORRECT_CURSOR")?;
            parsed_popularity = p;
            parsed_post_id = pid;
            // $3 = popularity, $4 = post_id
            sql += "
                AND (
                    (popularity_score) < $3 OR
                    ((popularity_score) = $3 AND post_id < $4)
                )
            ";
            params.push(&parsed_popularity);
            params.push(&parsed_post_id);
        }
        (PostsMode::New | PostsMode::ByFollowing, Some(c)) => {
            parsed_post_id = c.parse().map_err(|_| "INCORRECT_CURSOR")?;
            // $3 = post_id
            sql += " AND post_id < $3";
            params.push(&parsed_post_id);
        }
        _ => {}
    }

    sql += match mode {
        PostsMode::Popular => " ORDER BY popularity_score DESC, post_id DESC LIMIT $1",
        PostsMode::New | PostsMode::ByFollowing => " ORDER BY post_id DESC LIMIT $1",
    };

    let client = conn.get_client().await.unwrap();
    let rows = client.query(&sql, &params).await.unwrap();

    let next_cursor: Option<String> = match rows.last() {
        Some(r) => match mode {
            PostsMode::Popular => {
                let popularity: i64 = r.get("popularity_score");
                let post_id: i64 = r.get("post_id");
                Some(format!("{},{}", popularity, post_id))
            }
            PostsMode::New | PostsMode::ByFollowing => {
                let post_id: i64 = r.get("post_id");
                Some(post_id.to_string())
            }
        },
        None => None,
    };

    let posts: Vec<i64> = flatten_rows(rows, "post_id");
    Ok(PostsList { posts, next_cursor })
}
