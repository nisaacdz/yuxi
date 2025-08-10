use anyhow::anyhow;
use axum::{
    Extension, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use sea_orm::TryIntoModel;

use app::persistence::users::{create_user, get_user};
use app::state::AppState;
use app::{error::UserError, persistence::users::update_user};
use models::schemas::user::UserSchema;
use models::{
    params::user::{CreateUserParams, UpdateUserParams},
    schemas::user::AuthSchema,
};

use crate::error::ApiError;
use crate::extractor::{Json, Valid};

use super::auth::me_get;

async fn users_post(
    state: State<AppState>,
    Valid(Json(params)): Valid<Json<CreateUserParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user = create_user(&state, params).await.map_err(ApiError::from)?;

    let user = user.try_into_model().unwrap();
    Ok((StatusCode::CREATED, Json(UserSchema::from(user))))
}

async fn users_id_get(
    state: State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let user = get_user(&state, &id).await.map_err(ApiError::from)?;

    user.map(|user| Json(UserSchema::from(user)))
        .ok_or_else(|| UserError::NotFound.into())
}

#[axum::debug_handler]
async fn current_user_update(
    state: State<AppState>,
    Extension(auth_state): Extension<AuthSchema>,
    Valid(Json(params)): Valid<Json<UpdateUserParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = &auth_state
        .user
        .as_ref()
        .ok_or_else(|| anyhow!("User not logged in"))?
        .id;

    let updated_user = update_user(&state, user_id, params)
        .await
        .map_err(ApiError::from)?;

    let updated_user = updated_user.try_into_model()?;

    Ok((StatusCode::CREATED, Json(UserSchema::from(updated_user))))
}

pub fn create_user_router() -> Router<AppState> {
    Router::new()
        .route("/", post(users_post))
        .route("/{id}", get(users_id_get))
        .route("/me", get(me_get))
        .route("/me", patch(current_user_update))
}
