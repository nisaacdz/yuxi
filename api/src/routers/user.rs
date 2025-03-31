use anyhow::anyhow;
use axum::{
    extract::{FromRequest, Path, Query, Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Router,
};
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

use crate::error::ApiError;
use crate::extractor::{Json, Valid};

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
    req: Request,
) -> Result<impl IntoResponse, ApiError> {
    let user_session = req
        .extensions()
        .get::<ClientSchema>()
        .ok_or_else(|| anyhow!("Client Session not set"))?;

    let user_id = user_session
        .user
        .as_ref()
        .ok_or_else(|| anyhow!("User not logged in"))?
        .id;

    let Valid(Json(params)): Valid<Json<UpdateUserParams>> = Valid::from_request(req, &state)
        .await
        .map_err(|_| anyhow!("Invalid request body"))?;

    let updated_user = update_user(&state.conn, user_id, params)
        .await
        .map_err(ApiError::from)?;

    Ok((StatusCode::CREATED, Json(UserSchema::from(updated_user))))
}

pub fn create_user_router() -> Router<AppState> {
    Router::new()
        .route("/users", post(users_post).get(users_get))
        .route("/users/{id}", get(users_id_get))
        .route("/users/me", get(me_get))
        .route("/users/me/update", patch(current_user_update))
}
