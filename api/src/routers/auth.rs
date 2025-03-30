use anyhow::anyhow;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};

use app::persistence::users::get_user;
use app::state::AppState;
use app::{
    error::UserError,
    persistence::users::{create_user, login_user},
};
use models::schemas::user::UserSession;
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};
use sea_orm::TryIntoModel;

use crate::error::ApiError;
use crate::extractor::Json;

#[axum::debug_handler]
pub async fn login_post(
    state: State<AppState>,
    req: Request,
) -> Result<impl IntoResponse, ApiError> {
    // Split the request into parts and body
    let (mut parts, body) = req.into_parts();

    // Extract and clone the existing session
    let user_session = parts
        .extensions
        .get_mut::<UserSession>()
        .ok_or_else(|| anyhow!("Client Session not set"))?;

    // Extract and parse the body
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(ApiError::from)?;

    let params: LoginUserParams = serde_json::from_slice(&bytes).map_err(ApiError::from)?;

    let user = login_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;
    user_session.user = Some(UserSchema::from(user));
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn register_post(
    state: State<AppState>,
    req: Request,
) -> Result<impl IntoResponse, ApiError> {
    // Split the request into parts and body
    let (mut parts, body) = req.into_parts();

    // Extract and clone the existing session
    let user_session = parts
        .extensions
        .get_mut::<UserSession>()
        .ok_or_else(|| anyhow!("Client Session not set"))?;

    // Extract and parse the body
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(ApiError::from)?;

    let params: CreateUserParams = serde_json::from_slice(&bytes).map_err(ApiError::from)?;

    // Your business logic
    let user = create_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?
        .try_into_model()
        .unwrap();

    // Update the session
    user_session.user = Some(UserSchema::from(user));

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn logout_post(mut req: Request) -> Result<impl IntoResponse, ApiError> {
    let _user_schema = req
        .extensions_mut()
        .get_mut::<UserSession>()
        .map(|s| s.user.take())
        .flatten();
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn me_get(state: State<AppState>, req: Request) -> Result<impl IntoResponse, ApiError> {
    let user = if let Some(user) = &req.extensions().get::<UserSession>().unwrap().user {
        get_user(&state.conn, user.id)
            .await
            .map_err(ApiError::from)?
    } else {
        None
    };
    Ok(Json(user.map(|user| UserSchema::from(user))))
}

pub fn create_auth_router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login_post))
        .route("/auth/logout", post(logout_post))
        .route("/auth/register", post(register_post))
        .route("/auth/me", get(me_get))
}
