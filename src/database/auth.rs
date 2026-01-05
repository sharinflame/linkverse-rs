use crate::{
    database::conn::LazyConn,
    entities::user::AuthUser,
    utils::{
        security::{generate_key, generate_token, store_password_async},
        thread_state::generate_id,
    },
};
use deadpool_postgres::Transaction;
use serde::Serialize;
use tokio_postgres::Row;

#[derive(Debug, Serialize)]
pub struct Tokens {
    refresh: String,
    access: String,
}

/// Getting user using where_clause, private but used in public funcs
async fn get_user_by(
    conn: &mut LazyConn,
    query_param: &(dyn tokio_postgres::types::ToSql + Sync),
    where_clause: &str,
) -> Option<AuthUser> {
    let db = conn.get_client().await.unwrap();
    let sql = format!(
        "
        SELECT username, user_id, email, password_hash,
               email_verified, pending_email,
               EXTRACT(EPOCH FROM pending_email_until)::BIGINT AS pending_email_until
        FROM users
        WHERE {}
        ",
        where_clause
    );

    let row = db.query_opt(&sql, &[query_param]).await.unwrap();

    row.map(row_to_auth_user)
}

fn row_to_auth_user(row: Row) -> AuthUser {
    AuthUser {
        username: row.get("username"),
        user_id: row.get("user_id"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        email_verified: row.get("email_verified"),
        pending_email: row.get("pending_email"),
        pending_email_until: row.get("pending_email_until"),
    }
}

/// Get auth user by user_id
pub async fn get_auth_user(user_id: &i64, conn: &mut LazyConn) -> Option<AuthUser> {
    get_user_by(conn, user_id, "user_id = $1").await
}

/// Get auth user by email
pub async fn get_auth_user_by_email(email: &String, conn: &mut LazyConn) -> Option<AuthUser> {
    get_user_by(conn, email, "email = $1").await
}

/// Creates refresh and access tokens for user_id
pub async fn create_tokens(user_id: i64, tx: &mut Transaction<'_>) -> Tokens {
    let new_secret = generate_key(16);
    let new_session_id = generate_id();

    let refresh = generate_token(&user_id, "refresh", true, &new_secret, &new_session_id).await;

    let access = generate_token(&user_id, "access", false, &new_secret, &new_session_id).await;

    tx.execute(
        "
        INSERT INTO auth_keys (user_id, token_secret, session_id)
        VALUES ($1, $2, $3)
        ",
        &[&user_id, &new_secret, &new_session_id],
    )
    .await
    .unwrap();

    Tokens { refresh, access }
}

/// Updates refresh and access tokens for session_id
pub async fn update_tokens(user_id: i64, session_id: i64, tx: &mut Transaction<'_>) -> Tokens {
    let new_secret = generate_key(16);

    let refresh = generate_token(&user_id, "refresh", true, &new_secret, &session_id).await;

    let access = generate_token(&user_id, "access", false, &new_secret, &session_id).await;

    tx.execute(
        "
        UPDATE auth_keys
        SET token_secret = $1
        WHERE session_id = $2
        ",
        &[&new_secret, &session_id],
    )
    .await
    .unwrap();

    Tokens { refresh, access }
}

/// Check if user with email already exists
pub async fn email_exists(email: &String, conn: &mut LazyConn) -> bool {
    let db = conn.get_client().await.unwrap();

    let value = db
        .query_opt(
            "
            SELECT 1 FROM users
            WHERE email = $1 OR pending_email = $1
            LIMIT 1
            ",
            &[email],
        )
        .await
        .unwrap();
    value.is_some()
}

/// Check username
pub async fn username_exists(username: &String, conn: &mut LazyConn) -> bool {
    let db = conn.get_client().await.unwrap();

    let value = db
        .query_opt(
            "
            SELECT 1 FROM users
            WHERE username = $1
            LIMIT 1
            ",
            &[username],
        )
        .await
        .unwrap();
    value.is_some()
}

/// Create new user
/// Consider checking username and email existence before using this func
pub async fn create_user(
    username: &String,
    email: &String,
    password: String,
    tx: &mut Transaction<'_>,
) -> i64 {
    let new_user_id = generate_id();
    let password_hash = store_password_async(password).await;
    tx.execute(
        "
        INSERT INTO users (user_id, username, email, password_hash)
        VALUES ($1, $2, $3, $4)
        ",
        &[&new_user_id, username, email, &password_hash],
    )
    .await
    .unwrap();
    new_user_id
}

/// Check user session secret
pub async fn check_session_secret(
    user_id: &i64,
    session_id: &i64,
    secret: &String,
    conn: &mut LazyConn,
) -> bool {
    let db = conn.get_client().await.unwrap();

    let value = db
        .query_opt(
            "
            SELECT 1 FROM auth_keys
            WHERE user_id = $1
            AND session_id = $2
            AND token_secret = $3
            LIMIT 1
            ",
            &[user_id, session_id, secret],
        )
        .await
        .unwrap();
    value.is_some()
}

/// Deletes session
pub async fn remove_session(session_id: &i64, user_id: &i64, tx: &mut Transaction<'_>) {
    tx.execute(
        "
        DELETE FROM auth_keys
        WHERE user_id = $1 AND session_id = $2;
        ",
        &[user_id, session_id],
    )
    .await
    .unwrap();
}
