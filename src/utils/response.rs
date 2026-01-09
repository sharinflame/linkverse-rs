use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ApiResponseData<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

#[derive(Debug)]
pub struct ApiResponse<T> {
    data: ApiResponseData<T>,
    status: StatusCode,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T, status: StatusCode) -> Self {
        let data = ApiResponseData {
            success: true,
            data: Some(data),
            error: None,
        };
        Self { data, status }
    }

    pub fn err(data: Option<T>, error: &'static str, status: StatusCode) -> Self {
        let data = ApiResponseData {
            success: false,
            data,
            error: Some(error),
        };
        Self { data, status }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        let json = axum::Json(self.data);
        (self.status, json).into_response()
    }
}

#[derive(Debug)]
pub enum AppError {
    NotFound(&'static str),
    Unauthorized(&'static str),
    BadRequest(&'static str),
    Internal(&'static str),
    Forbidden(&'static str),
    Conflict(&'static str),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
        };

        let body = Json(ApiResponseData::<()> {
            success: false,
            data: None,
            error: Some(error_message),
        });

        (status, body).into_response()
    }
}

#[derive(Debug)]
pub enum FuncError {
    UserNotFound,
    IncorrectPassword,
    IncorrectData,
    IncorrectParams,
    UserAlreadyExists,
    UsernameExists,
    InternalServerError,
    Unauthorized,
    ExpiredToken,
    InvalidToken,
    PostDoesNotExist,
    NoMorePosts
}

impl From<FuncError> for AppError {
    fn from(err: FuncError) -> Self {
        match err {
            FuncError::UserNotFound => AppError::NotFound("USER_NOT_FOUND"),
            FuncError::IncorrectPassword => AppError::Unauthorized("INCORRECT_PASSWORD"),
            FuncError::IncorrectData => AppError::BadRequest("INCORRECT_DATA"),
            FuncError::UserAlreadyExists => AppError::Conflict("USER_ALREADY_EXISTS"),
            FuncError::UsernameExists => AppError::Conflict("USERNAME_EXISTS"),
            FuncError::InternalServerError => AppError::Internal("INTERNAL_SERVER_ERROR"),
            FuncError::Unauthorized => AppError::Unauthorized("UNAUTHORIZED"),
            FuncError::ExpiredToken => AppError::Unauthorized("EXPIRED_TOKEN"),
            FuncError::InvalidToken => AppError::Unauthorized("INVALID_TOKEN"),
            FuncError::PostDoesNotExist => AppError::NotFound("POST_DOES_NOT_EXIST"),
            FuncError::IncorrectParams => AppError::BadRequest("INCORRECT_PARAMS"),
            FuncError::NoMorePosts => AppError::BadRequest("NO_MORE_POSTS")
        }
    }
}

pub fn response<T>(data: T, status: StatusCode) -> ApiResponse<T> {
    ApiResponse::<T>::ok(data, status)
}
