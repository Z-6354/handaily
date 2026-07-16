use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiOk<T: Serialize> {
    pub ok: bool,
    pub data: T,
}

#[derive(Serialize)]
pub struct ApiErrBody {
    pub ok: bool,
    pub error: ApiErrorDetail,
}

#[derive(Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

pub fn ok<T: Serialize>(data: T) -> (StatusCode, Json<ApiOk<T>>) {
    (
        StatusCode::OK,
        Json(ApiOk {
            ok: true,
            data,
        }),
    )
}

pub fn ok_status<T: Serialize>(status: StatusCode, data: T) -> (StatusCode, Json<ApiOk<T>>) {
    (
        status,
        Json(ApiOk {
            ok: true,
            data,
        }),
    )
}

pub fn err(status: StatusCode, code: &str, message: impl Into<String>) -> (StatusCode, Json<ApiErrBody>) {
    (
        status,
        Json(ApiErrBody {
            ok: false,
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
            },
        }),
    )
}

pub fn err_response(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    err(status, code, message).into_response()
}
