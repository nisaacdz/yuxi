use app::persistence::users::{create_user, login_user};
use app::state::AppState;
use axum::{
    Extension, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};
use sea_orm::TryIntoModel;
use tower_sessions::Session;

use crate::extractor::Json;
use crate::{error::ApiError, middleware::session::CLIENT_SESSION_KEY};

#[axum::debug_handler]
pub async fn login_post(
    State(state): State<AppState>,
    session: Session,
    Extension(client): Extension<ClientSchema>,
    Json(params): Json<LoginUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = login_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;
    let client = ClientSchema {
        client_id: client.client_id,
        user: Some(UserSchema::from(user)),
        updated: Utc::now(),
    };

    session
        .insert(CLIENT_SESSION_KEY, &client)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert new client session data: {}", e);
            ApiError(anyhow::anyhow!("Failed to save session state").context(e))
        })?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn register_post(
    State(state): State<AppState>,
    session: Session,
    Extension(client): Extension<ClientSchema>,
    Json(params): Json<CreateUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user_db = create_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?
        .try_into_model()
        .map_err(ApiError::from)?;

    let updated_client_state = ClientSchema {
        client_id: client.client_id,
        user: Some(UserSchema::from(user_db)),
        updated: Utc::now(),
    };

    session
        .insert(CLIENT_SESSION_KEY, &updated_client_state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert session data after registration: {}", e);
            ApiError(anyhow::anyhow!("Failed to save session state").context(e))
        })?;

    Ok(StatusCode::CREATED)
}

#[axum::debug_handler]
pub async fn logout_post(
    session: Session,
    Extension(mut client): Extension<ClientSchema>,
) -> Result<impl IntoResponse, ApiError> {
    if client.user.is_none() {
        return Ok(StatusCode::OK);
    }

    client.user = None;
    client.updated = Utc::now();

    session
        .insert(CLIENT_SESSION_KEY, &client)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert session data after logout: {}", e);
            ApiError(anyhow::anyhow!("Failed to save session state").context(e))
        })?;
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn me_get(
    Extension(client_state): Extension<ClientSchema>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(client_state))
}

pub fn create_auth_router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login_post))
        .route("/auth/logout", post(logout_post))
        .route("/auth/register", post(register_post))
        .route("/auth/me", get(me_get))
}
