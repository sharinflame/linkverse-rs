use axum::{Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use crate::{
    create_tx,
    database::conn::LazyConn,
    entities::user::User,
    extractors::auth::AuthSession,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        state::ArcAppState,
    },
};

mod me {

    use crate::{
        database::users::get_user,
        utils::{
            perms::{permissions_to_list, role_permissions},
            snowflake::created_at,
        },
    };

    use super::*;

    // Get current user
    #[derive(Debug, Serialize)]
    pub struct Returns {
        #[serde(flatten)]
        pub user: User,
        pub created_at: f64,
        pub permissions: Vec<&'static str>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);
        let user = get_user(&session.user_id, &mut conn)
            .await
            .ok_or(FuncError::UserNotFound)?;

        let perms = role_permissions(&user.role_id);
        let permissions = permissions_to_list(perms);

        Ok(response(
            Returns {
                created_at: created_at(user.user_id.parse().unwrap()),
                user,
                permissions,
            },
            StatusCode::OK,
        ))
    }
}

mod patch_me {
    use serde::Deserialize;
    use validator::{Validate, ValidationError};

    use super::*;
    use crate::{
        database::users::{UserProfileUpdate, update_user_profile},
        map_struct,
        utils::validate::ValidatedJson,
    };

    fn validate_languages(langs: &Vec<String>) -> Result<(), ValidationError> {
        if langs.len() > 8 {
            return Err(ValidationError::new("too_many_languages"));
        }
        for lang in langs {
            if lang.len() > 16 {
                return Err(ValidationError::new("language_too_long"));
            }
        }
        Ok(())
    }

    // Patch current user's profile
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 0, max = 16))]
        pub display_name: Option<String>,
        #[validate(length(min = 0, max = 32))]
        pub avatar_context_id: Option<String>,
        #[validate(length(min = 0, max = 32))]
        pub banner_context_id: Option<String>,
        #[validate(length(min = 0, max = 512))]
        pub bio: Option<String>,
        #[validate(custom(function = "validate_languages"))]
        pub languages: Option<Vec<String>>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);
        let mut tx = create_tx!(conn);

        // We convert PatchPayload to UserProfileUpdate so we can validate
        // Validation has to be in endpoints/ not in database/ so we gotta do this here
        // That's my choice
        let mut dirty = false;

        dirty |= update_user_profile(
            &session.user_id,
            map_struct!(payload => UserProfileUpdate {
                display_name,
                avatar_context_id,
                banner_context_id,
                bio,
                languages,
            }),
            &mut tx,
        )
        .await;

        if dirty {
            tx.commit().await.unwrap();
        }

        Ok(StatusCode::NO_CONTENT)
    }
}

mod get_user {
    use axum::extract::Path;

    use crate::{database::users::get_user, utils::snowflake::created_at};

    use super::*;

    #[derive(Debug, Serialize)]
    pub struct Returns {
        #[serde(flatten)]
        pub user: User,
        pub created_at: f64,
    }

    pub async fn handler(
        _session: AuthSession,
        State(state): State<ArcAppState>,
        Path(user_id): Path<String>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);
        let user = get_user(&user_id, &mut conn)
            .await
            .ok_or(FuncError::UserNotFound)?;

        Ok(response(
            Returns {
                created_at: created_at(user.user_id.parse().unwrap()),
                user,
            },
            StatusCode::OK,
        ))
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route("/me", get(me::handler).patch(patch_me::handler))
        .route("/{user_id}", get(get_user::handler))
}
