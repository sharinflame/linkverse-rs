use std::collections::HashMap;

use axum::{Router, extract::State, http::StatusCode, routing::post};
use once_cell::sync::Lazy;
use serde::Deserialize;
use validator::Validate;

use crate::{
    create_tx,
    database::conn::LazyConn,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, response},
        state::ArcAppState,
        validate::ValidatedJson,
    },
};

static LIMITS: Lazy<HashMap<String, i32>> = Lazy::new(|| {
    HashMap::from([
        ("avatar".to_string(), 2),
        ("banner".to_string(), 8),
        ("post_video".to_string(), 15),
        ("post_image".to_string(), 10),
    ])
});

static MAX_COUNT: Lazy<HashMap<String, i32>> =
    Lazy::new(|| HashMap::from([("post_video".to_string(), 1), ("post_image".to_string(), 5)]));

mod create_context {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde::Serialize;
    use serde_with::{DisplayFromStr, serde_as};
    use validator::ValidationError;

    use crate::{database::storage::create_file_context, extractors::auth::AuthSession};

    use super::*;

    fn validate_type(r#type: &String) -> Result<(), ValidationError> {
        if r#type != "post_video" && r#type != "post_image" {
            return Err(ValidationError::new("wrong_type"));
        }
        Ok(())
    }

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(custom(function = "validate_type"))]
        r#type: String,
    }

    #[serde_as]
    #[derive(Serialize, Debug)]
    pub struct Returns {
        #[serde_as(as = "DisplayFromStr")]
        pub context_id: i64,
        pub max_size: i32,
        pub max_count: i32,
        pub expires: u64,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);

        let max_count = *MAX_COUNT.get(&payload.r#type).unwrap();
        let max_size = *LIMITS.get(&payload.r#type).unwrap();

        // Creating context
        let mut tx = create_tx!(conn);
        let context_id = create_file_context(
            &session.user_id,
            &Vec::new(),
            &max_count,
            &payload.r#type,
            &mut tx,
        )
        .await;
        tx.commit().await.unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        return Ok(response(
            Returns {
                context_id: context_id,
                max_size: max_size,
                max_count: max_count,
                expires: now + 30 * 60,
            },
            StatusCode::CREATED,
        ));
    }
}

mod upload_file {
    use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SAFE: &AsciiSet = &CONTROLS.add(b'/');

    use serde::Serialize;
    use serde_with::{DisplayFromStr, serde_as};
    use validator::ValidationError;

    use crate::{
        database::storage::{append_file, create_file_context, get_file_context},
        extractors::auth::AuthSession,
        utils::{
            response::FuncError,
            storage::{Operation, PUBLIC_PATH, generate_signed_token},
        },
    };

    use super::*;

    fn validate_type(r#type: &String) -> Result<(), ValidationError> {
        if r#type != "banner" && r#type != "avatar" && r#type != "context" {
            return Err(ValidationError::new("wrong_type"));
        }
        Ok(())
    }

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[serde_as(as = "Option<DisplayFromStr>")]
        pub context_id: Option<i64>,
        #[validate(custom(function = "validate_type"))]
        r#type: String,
        #[validate(length(min = 1, max = 92))]
        file_name: String,
    }

    #[serde_as]
    #[derive(Serialize, Debug)]
    pub struct Returns {
        #[serde_as(as = "DisplayFromStr")]
        pub context_id: i64,
        pub file_url: String,
        pub file_name: String,
        pub headers: HashMap<String, String>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);
        let mut r#type = payload.r#type;
        let context_id: i64;
        let file_name: String;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let mut tx;
        if r#type == "context" {
            context_id = payload.context_id.ok_or(FuncError::IncorrectData)?;

            let context = get_file_context(&context_id, &mut conn)
                .await
                .ok_or(FuncError::ContextNotFound)?;
            if context.user_id != session.user_id || now - 60 * 60 > context.created_at as u64 {
                return Err(FuncError::Forbidden.into());
            }

            tx = create_tx!(conn);
            r#type = context.r#type;
            file_name = format!(
                "private/{}/{}",
                context_id,
                utf8_percent_encode(&payload.file_name, SAFE).to_string()
            );
        } else {
            let subfolder = if r#type == "avatar" {
                "avatars"
            } else {
                "banners"
            };

            tx = create_tx!(conn);
            context_id =
                create_file_context(&session.user_id, &Vec::new(), &1, &r#type, &mut tx).await;

            file_name = format!(
                "public/{}/{}/{}.webp",
                subfolder, session.user_id, context_id
            );
        }
        append_file(&context_id, &file_name, &mut tx)
            .await
            .map_err(AppError::BadRequest)?;
        tx.commit().await.unwrap();

        let max_size = *LIMITS.get(&r#type).unwrap();
        let token = generate_signed_token(
            &[(Operation::GET, &file_name), (Operation::PUT, &file_name)],
            900,
            Some(max_size.try_into().unwrap()),
            Some(&r#type),
        );

        return Ok(response(
            Returns {
                file_url: format!("{}/{}", PUBLIC_PATH, file_name),
                file_name: file_name,
                context_id,
                headers: HashMap::from([("X-Custom-Auth".to_string(), token)]),
            },
            StatusCode::CREATED,
        ));
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route("/v1/storage/context", post(create_context::handler))
        .route("/v1/storage/file", post(upload_file::handler))
}
