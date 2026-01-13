use deadpool_postgres::Transaction;
use tokio_postgres::{Row, types::ToSql};

use crate::{
    database::conn::LazyConn,
    entities::user::User,
    utils::{
        perms::{Permission, role_permissions},
        storage::normalize_url,
    },
};

/// Private function for converting Row to User
fn row_to_user(row: Row) -> User {
    User {
        user_id: row.get("user_id"),
        username: row.get("username"),
        role_id: row.get("role_id"),
        display_name: row.get("display_name"),
        avatar_url: normalize_url(row.get("avatar_url")),
        banner_url: normalize_url(row.get("banner_url")),
        bio: row.get("bio"),
        badges: row.get("badges"),
        languages: row.get("languages"),
        following_count: row.get("following_count"),
        followers_count: row.get("followers_count"),
    }
}

/// Get minimized user from database
pub async fn get_min_user(user_id: &i64, conn: &mut LazyConn) -> Option<User> {
    let db = conn.get_client().await.unwrap();
    let sql = "
        SELECT u.user_id, u.username, p.display_name, u.role_id,
               ac.objects[1] as avatar_url,
               NULL::text as banner_url,
               NULL::text as bio,
               NULL::smallint[] as badges,
               NULL::text[] as languages,
               NULL::bigint as following_count,
               NULL::bigint as followers_count
        FROM users u
        LEFT JOIN user_profiles p ON u.user_id = p.user_id
        LEFT JOIN files ac ON ac.context_id = p.avatar_context_id
        WHERE u.user_id = $1;
    ";
    let row = db.query_opt(sql, &[user_id]).await.unwrap();
    row.map(row_to_user)
}

/// Get full user from database
pub async fn get_user(user_id: &i64, conn: &mut LazyConn) -> Option<User> {
    let db = conn.get_client().await.unwrap();
    let sql = "
        SELECT u.user_id, u.username, p.display_name, u.role_id,
               ac.objects[1] as avatar_url,
               bc.objects[1] as banner_url, p.bio, p.badges, p.languages,
               u.following_count, u.followers_count
        FROM users u
        LEFT JOIN user_profiles p ON u.user_id = p.user_id
        LEFT JOIN files ac ON ac.context_id = p.avatar_context_id
        LEFT JOIN files bc ON bc.context_id = p.banner_context_id
        WHERE u.user_id = $1;
    ";
    let row = db.query_opt(sql, &[user_id]).await.unwrap();
    row.map(row_to_user)
}

#[derive(Default, Debug)]
pub struct UserProfileUpdate {
    pub display_name: Option<String>,
    pub avatar_context_id: Option<i64>,
    pub banner_context_id: Option<i64>,
    pub bio: Option<String>,
    pub languages: Option<Vec<String>>,
}

/// Private function for 'update_user_profile'
fn push_opt<'a, T: tokio_postgres::types::ToSql + Sync>(
    opt: &'a Option<T>,
    column: &'static str,
    columns: &mut Vec<&'static str>,
    values: &mut Vec<&'a (dyn tokio_postgres::types::ToSql + Sync)>,
) {
    if let Some(v) = opt.as_ref() {
        columns.push(column);
        values.push(v);
    }
}

/// Updates user profile
pub async fn update_user_profile(
    user_id: &i64,
    update: UserProfileUpdate,
    tx: &mut Transaction<'_>,
) -> bool {
    let mut columns: Vec<&str> = Vec::new();
    let mut values: Vec<&(dyn ToSql + Sync)> = Vec::new();

    push_opt(
        &update.display_name,
        "display_name",
        &mut columns,
        &mut values,
    );
    push_opt(
        &update.avatar_context_id,
        "avatar_context_id",
        &mut columns,
        &mut values,
    );
    push_opt(
        &update.banner_context_id,
        "banner_context_id",
        &mut columns,
        &mut values,
    );
    push_opt(&update.bio, "bio", &mut columns, &mut values);
    push_opt(&update.languages, "languages", &mut columns, &mut values);

    if columns.is_empty() {
        return false;
    }

    // columns like "display_name, avatar_context_id, ..."
    let columns_str = columns.join(", ");

    // placeholders $2, $3, ... ($1 reserved for user_id)
    let placeholders: Vec<String> = (0..values.len()).map(|i| format!("${}", i + 2)).collect();
    let placeholders_str = placeholders.join(", ");

    // update clause: col = EXCLUDED.col, ...
    let update_clause: Vec<String> = columns
        .iter()
        .map(|c| format!("{} = EXCLUDED.{}", c, c))
        .collect();
    let update_clause_str = update_clause.join(", ");

    let query = format!(
        "INSERT INTO user_profiles (user_id, {}) VALUES ($1, {}) \
         ON CONFLICT (user_id) DO UPDATE SET {}",
        columns_str, placeholders_str, update_clause_str
    );

    let mut params: Vec<&(dyn ToSql + Sync)> = vec![&user_id];
    params.extend(values);

    // execute
    tx.execute(query.as_str(), &params).await.unwrap();
    true
}

/// Follow user
/// 'user_id' is user that will start following 'target_id'
/// If user was already followed, nothing happens
pub async fn follow_user(user_id: &i64, target_id: &i64, tx: &mut Transaction<'_>) {
    tx.execute(
        "
        INSERT INTO followed (user_id, followed_to)
        VALUES ($1, $2)
        ON CONFLICT (user_id, followed_to) DO NOTHING
        ",
        &[user_id, target_id],
    )
    .await
    .unwrap();
}

/// Unfollow user
/// 'user_id' is user that will stop following 'target_id'
/// If user wasn't followed, nothing happens
pub async fn unfollow_user(user_id: &i64, target_id: &i64, tx: &mut Transaction<'_>) {
    tx.execute(
        "
        DELETE FROM followed
        WHERE user_id = $1 AND followed_to = $2
        ",
        &[user_id, target_id],
    )
    .await
    .unwrap();
}

// Will return true if 'user_id' is following 'target_id'
pub async fn is_followed(user_id: &i64, target_id: &i64, conn: &mut LazyConn) -> bool {
    let db = conn.get_client().await.unwrap();
    let value = db
        .query_opt(
            "
            SELECT 1
            FROM followed
            WHERE user_id = $1 AND followed_to = $2
            ",
            &[user_id, target_id],
        )
        .await
        .unwrap();
    value.is_some()
}

/// Get Permission object for user
pub async fn get_permissions(user_id: &i64, conn: &mut LazyConn) -> Permission {
    let db = conn.get_client().await.unwrap();
    let row = db
        .query_opt(
            "
            SELECT role_id
            FROM users
            WHERE user_id = $1
            ",
            &[user_id],
        )
        .await
        .unwrap();
    let role_id: i32;
    if let Some(row) = row {
        role_id = row.get("role_id");
    } else {
        role_id = 0;
    }

    role_permissions(&role_id)
}
