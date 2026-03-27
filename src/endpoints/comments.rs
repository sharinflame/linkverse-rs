use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use validator::{Validate, ValidationError};

use crate::{
    create_tx,
    database::conn::LazyConn,
    extractors::auth::AuthSession,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        state::ArcAppState,
        validate::ValidatedJson,
    },
};

fn validate_type(r#type: &String) -> Result<(), ValidationError> {
    if r#type != "comment" && r#type != "update" {
        return Err(ValidationError::new("invalid_type"));
    }

    Ok(())
}

mod get_comment {
    use axum::extract::Path;

    use crate::views::for_user::{ForUserCommentView, get_full_comment_by_id};

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(comment_id): Path<i64>,
    ) -> Result<ApiResponse<ForUserCommentView>, AppError> {
        let mut conn = get_conn!(state);
        let comment = get_full_comment_by_id(&comment_id, &session.user_id, &mut conn)
            .await
            .ok_or(FuncError::CommentDoesNotExist)?;

        Ok(response(comment, StatusCode::OK))
    }
}

mod create_comment {
    use axum::extract::Path;

    use crate::{database::comments::create_comment, entities::comment::Comment};

    use super::*;

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 0, max = 1024))]
        content: String,

        #[validate(custom(function = "validate_type"))]
        r#type: Option<String>,

        #[serde_as(as = "Option<DisplayFromStr>")]
        parent_id: Option<i64>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(post_id): Path<i64>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Comment>, AppError> {
        let mut conn = get_conn!(state);

        let mut tx = create_tx!(conn);
        let comment = create_comment(
            &post_id,
            &session.user_id,
            &payload.content,
            payload.parent_id,
            &mut tx,
        )
        .await;
        tx.commit().await.unwrap();

        return Ok(response(comment, StatusCode::CREATED));
    }
}

mod get_comments {
    use axum::extract::Path;
    use serde::Serialize;

    use crate::{
        database::comments::get_comments,
        utils::validate::ValidatedQuery,
        views::for_user::{ForUserCommentView, get_full_comment_by_id},
    };

    use super::*;

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 0, max = 256))]
        cursor: Option<String>,

        #[validate(custom(function = "validate_type"))]
        r#type: Option<String>,

        #[serde_as(as = "Option<DisplayFromStr>")]
        parent_id: Option<i64>,
    }

    #[derive(Debug, Serialize)]
    pub struct Returns {
        comments: Vec<ForUserCommentView>,
        next_cursor: Option<String>,
        has_more: bool,
    }

    async fn load_comments_with_replies(
        post_id: &i64,
        parent_id: &Option<i64>,
        user_id: &i64,
        conn: &mut LazyConn,
        depth: i64,
        max_depth: i64,
        cursor: &Option<String>,
        r#type: &Option<String>,
    ) -> Result<Vec<ForUserCommentView>, &'static str> {
        if depth > max_depth {
            return Ok(Vec::new());
        }
        let result = get_comments(post_id, cursor, user_id, conn, r#type, parent_id, &3).await?;

        let mut replies: Vec<ForUserCommentView> = Vec::new();

        for comment in result.comments {
            if let Some(mut full_comment) = get_full_comment_by_id(&comment, user_id, conn).await {
                full_comment.replies = Some(
                    Box::pin(load_comments_with_replies(
                        post_id,
                        &full_comment.comment.parent_commend_id,
                        user_id,
                        conn,
                        depth + 1,
                        max_depth,
                        cursor,
                        r#type,
                    ))
                    .await?,
                );
                replies.push(full_comment);
            }
        }

        Ok(replies)
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(post_id): Path<i64>,
        ValidatedQuery(payload): ValidatedQuery<Payload>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);
        let mut results: Vec<ForUserCommentView> = Vec::new();

        let comments = get_comments(
            &post_id,
            &payload.cursor,
            &session.user_id,
            &mut conn,
            &payload.r#type,
            &payload.parent_id,
            &20,
        )
        .await
        .map_err(AppError::BadRequest)?;

        for comment in &comments.comments {
            if let Some(mut full_comment) =
                get_full_comment_by_id(&comment, &session.user_id, &mut conn).await
            {
                full_comment.replies = Some(
                    load_comments_with_replies(
                        &post_id,
                        &full_comment.comment.parent_commend_id,
                        &session.user_id,
                        &mut conn,
                        0,
                        3,
                        &None,
                        &payload.r#type,
                    )
                    .await
                    .map_err(AppError::BadRequest)?,
                );
                results.push(full_comment)
            }
        }

        Ok(response(
            Returns {
                comments: results,
                has_more: comments.has_more,
                next_cursor: comments.next_cursor,
            },
            StatusCode::OK,
        ))
    }
}

mod delete_comment {
    use axum::extract::Path;

    use crate::{
        database::{
            comments::{get_comment, soft_delete_comment},
            users::get_permissions,
        },
        utils::perms::Permission,
    };

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(comment_id): Path<i64>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);
        let post = get_comment(&comment_id, &mut conn)
            .await
            .ok_or(FuncError::CommentDoesNotExist)?;

        if post.user_id != session.user_id {
            let perms = get_permissions(&session.user_id, &mut conn).await;
            if perms.contains(Permission::MODERATE_COMMENTS) {
                // TODO: Finish moderation deletion
                return Err(FuncError::NotImplemented.into());
            }
            return Err(FuncError::Forbidden.into());
        }

        let mut tx = create_tx!(conn);
        soft_delete_comment(&comment_id, &mut tx).await;
        tx.commit().await.unwrap();

        Ok(StatusCode::NO_CONTENT)
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route(
            "/v1/comments/{comment_id}",
            get(get_comment::handler).delete(delete_comment::handler),
        )
        .route(
            "/v1/posts/{post_id}/comments",
            post(create_comment::handler).get(get_comments::handler),
        )
}
