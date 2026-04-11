use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tokio::sync::Semaphore;
use tokio::time::interval;

use crate::database::conn::LazyConn;
use crate::database::storage::{delete_file_context, get_contexts_for_deletion};
use crate::utils::state::AppState;
use crate::utils::storage::{Operation, PUBLIC_PATH, generate_signed_token};
use crate::{create_tx, get_conn};

const BATCH_SIZE: i32 = 10_000;
const MAX_CONCURRENCY: usize = 100;

pub async fn start_cleanup_loop(app_state: Arc<AppState>) {
    let client = Client::new();

    let mut ticker = interval(Duration::from_secs(30 * 60));

    loop {
        ticker.tick().await;

        if let Err(err) = cleanup_files(app_state.clone(), &client).await {
            eprintln!("cleanup failed: {err}");
        }
    }
}

pub async fn delete_object(object_path: &str, client: &Client) -> Result<(), Box<dyn Error>> {
    let token = generate_signed_token(&[(Operation::DELETE, object_path)], 60, None, None);

    let url = format!("{}/{}", PUBLIC_PATH, object_path);

    let response = client
        .delete(url)
        .header("X-Custom-Auth", token)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Request failed: {}", response.status()).into());
    }

    Ok(())
}

pub async fn cleanup_files(
    app_state: Arc<AppState>,
    client: &Client,
) -> Result<bool, Box<dyn Error>> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));

    let mut conn = get_conn!(app_state);
    let rows = get_contexts_for_deletion(true, BATCH_SIZE, &mut conn).await;

    let mut tx = create_tx!(conn);

    for row in &rows {
        let context_id = &row.context_id;
        let objects: &Vec<String> = &row.objects;

        let mut handles = Vec::with_capacity(objects.len());

        for obj in objects {
            let permit = semaphore.clone().acquire_owned().await?;
            let client = client.clone();
            let obj = obj.clone();

            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let _ = delete_object(&obj, &client).await;
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        delete_file_context(context_id, &mut tx).await;
    }

    tx.commit().await?;

    Ok(!rows.is_empty())
}
