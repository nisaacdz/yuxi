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
use models::schemas::user::{ClientSchema, TokensSchema};
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};
use sea_orm::TryIntoModel;

use crate::extractor::Json;
use crate::{error::ApiError, utils::jwt::JwtService};

#[axum::debug_handler]
pub async fn login_post(
    State(state): State<AppState>,
    Extension(client): Extension<ClientSchema>,
    Json(params): Json<LoginUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = login_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;

    let jwt_service = JwtService::new()?;
    let token_pair = jwt_service.generate_token_pair(client.id, Some(user.id))?;

    let user_schema = UserSchema::from(user);
    let tokens_response = TokensSchema {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        user: Some(user_schema),
    };

    Ok(Json(tokens_response))
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
        id: client.id,
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

    Ok(Json(updated_client_state.user))
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
        .route("/login", post(login_post))
        .route("/logout", post(logout_post))
        .route("/register", post(register_post))
        .route("/me", get(me_get))
}
