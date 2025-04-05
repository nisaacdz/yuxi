use anyhow::anyhow;
use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use chrono::Utc;
use sea_orm::TryIntoModel;

use app::persistence::users::{create_user, get_user, search_users};
use app::state::AppState;
use app::{error::UserError, persistence::users::update_user};
use models::queries::user::UserQuery;
use models::schemas::user::{UserListSchema, UserSchema};
use models::{
    params::user::{CreateUserParams, UpdateUserParams},
    schemas::user::ClientSchema,
};
use tower_sessions::Session;

use crate::extractor::{Json, Valid};
use crate::{error::ApiError, middleware::session::CLIENT_SESSION_KEY};

use super::auth::me_get;

async fn users_post(
    state: State<AppState>,
    Valid(Json(params)): Valid<Json<CreateUserParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user = create_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;

    let user = user.try_into_model().unwrap();
    Ok((StatusCode::CREATED, Json(UserSchema::from(user))))
}

#[axum::debug_handler]
async fn users_get(
    State(state): State<AppState>,
    Query(query): Query<Option<UserQuery>>,
) -> Result<impl IntoResponse, ApiError> {
    let query = query.unwrap_or_default();

    let users = search_users(&state.conn, query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(UserListSchema::from(users)))
}

async fn users_id_get(
    state: State<AppState>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, ApiError> {
    let user = get_user(&state.conn, id).await.map_err(ApiError::from)?;

    user.map(|user| Json(UserSchema::from(user)))
        .ok_or_else(|| UserError::NotFound.into())
}

#[axum::debug_handler]
async fn current_user_update(
    state: State<AppState>,
    session: Session,
    Extension(client): Extension<ClientSchema>,
    Valid(Json(params)): Valid<Json<UpdateUserParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = client
        .user
        .as_ref()
        .ok_or_else(|| anyhow!("User not logged in"))?
        .id;

    let updated_user = update_user(&state.conn, user_id, params)
        .await
        .map_err(ApiError::from)?;

    let updated_user = updated_user.try_into_model()?;

    session
        .insert(
            CLIENT_SESSION_KEY,
            &ClientSchema {
                client_id: client.client_id,
                user: Some(UserSchema::from(updated_user.clone())),
                updated: Utc::now(),
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert new client session data: {}", e);
            ApiError(anyhow::anyhow!("Failed to insert new client session data"))
        })?;

    Ok((StatusCode::CREATED, Json(UserSchema::from(updated_user))))
}

pub fn create_user_router() -> Router<AppState> {
    Router::new()
        .route("/", post(users_post).get(users_get))
        .route("/{id}", get(users_id_get))
        .route("/me", get(me_get))
        .route("/me/update", patch(current_user_update))
}
