use deadpool_postgres::Transaction;
use tokio_postgres::Row;

use crate::{
    database::conn::LazyConn,
    entities::context::FileContext,
    utils::{state::CONFIG, thread_state::generate_id},
};

fn row_to_context(row: Row) -> FileContext {
    FileContext {
        context_id: row.get("context_id"),
        user_id: row.get("user_id"),
        objects: row.get("objects"),
        reference_count: row.get("reference_count"),
        allowed_count: row.get("allowed_count"),
        created_at: row.get("created_at"),
        r#type: row.get("type"),
    }
}

/// Creates context in database, it shows what files are linked to context
/// Context is useful way to connect list of files to any object
/// And by any object I mean avatar, banner, post, etc.
pub async fn create_file_context(
    user_id: &i64,
    objects: &Vec<String>,
    max_count: &i32,
    r#type: &String,
    tx: &mut Transaction<'_>,
) -> i64 {
    let new_id = generate_id();

    tx.execute(
        "
        INSERT INTO files
        (user_id, objects, allowed_count, context_id, type)
        VALUES ($1, $2, $3, $4, $5)
        ",
        &[user_id, objects, max_count, &new_id, r#type],
    )
    .await
    .unwrap();

    return new_id;
}

/// Deletes file context
/// Doesn't delete files in storage, should be called only when you're 100% sure that files are deleted
/// Imagine if there are 1000 files and you accidentally delete context, you will never know what files are needed
pub async fn delete_file_context(context_id: &i64, tx: &mut Transaction<'_>) {
    tx.execute(
        "
        DELETE FROM files
        WHERE context_id = $1
        ",
        &[context_id],
    )
    .await
    .unwrap();
}

/// Get file context
pub async fn get_file_context(context_id: &i64, conn: &mut LazyConn) -> Option<FileContext> {
    let db = conn.get_client().await.unwrap();
    let row = db
        .query_opt(
            "
            SELECT context_id, user_id, objects, reference_count, allowed_count,
                   EXTRACT(EPOCH FROM created_at)::bigint as created_at, type
            FROM files
            WHERE context_id = $1
            ",
            &[context_id],
        )
        .await
        .unwrap();
    row.map(row_to_context)
}

/// Appends file to the file context
/// If context not found will give error CONTEXT_NOT_FOUND
/// If num of objects > max allowed for the context will give error MAX_COUNT_EXCEED
pub async fn append_file(
    context_id: &i64,
    new_object: &String,
    tx: &mut Transaction<'_>,
) -> Result<(), &'static str> {
    let row = tx
        .query_opt(
            "
            SELECT objects, allowed_count
            FROM files
            WHERE context_id = $1
            FOR UPDATE
            ",
            &[context_id],
        )
        .await
        .unwrap()
        .ok_or("CONTEXT_NOT_FOUND")?;

    let mut allowed_count: i32 = row.get("allowed_count");
    if allowed_count <= 0 {
        return Err("MAX_COUNT_EXCEED");
    }

    let mut objects: Vec<String> = row.get("objects");

    allowed_count -= 1;
    objects.push(new_object.clone());

    tx.execute(
        "
        UPDATE files
        SET objects = $1,
            allowed_count = $2
        WHERE context_id = $3
        ",
        &[&objects, &allowed_count, context_id],
    )
    .await
    .unwrap();

    Ok(())
}

pub async fn get_contexts_for_deletion(
    use_server_id: bool,
    limit: i32,
    conn: &mut LazyConn,
) -> Vec<FileContext> {
    let client = conn.get_client().await.unwrap();

    let rows: Vec<Row>;
    if use_server_id {
        rows = client
            .query(
                "
                SELECT context_id, user_id, objects, reference_count, allowed_count,
                       EXTRACT(EPOCH FROM created_at)::bigint as created_at, type
                FROM files
                WHERE reference_count = 0
                    AND created_at < NOW() - INTERVAL '30 minutes'
                    AND (context_id::bigint % $2) = $1  -- server
                LIMIT $3
                ",
                &[
                    &(CONFIG.server_id as i64),
                    &(CONFIG.total_servers as i64),
                    &(limit as i64),
                ],
            )
            .await
            .unwrap();
    } else {
        rows = client
            .query(
                "
                SELECT context_id, user_id, objects, reference_count, allowed_count,
                       EXTRACT(EPOCH FROM created_at)::bigint as created_at, type
                FROM files
                WHERE reference_count = 0
                    AND created_at < NOW() - INTERVAL '30 minutes'
                LIMIT $1
                ",
                &[&limit],
            )
            .await
            .unwrap();
    }

    let mut contexts: Vec<FileContext> = Vec::new();

    for row in rows {
        contexts.push(row_to_context(row))
    }

    return contexts;
}
