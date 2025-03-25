use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use sea_orm::TryIntoModel;

use app::error::UserError;
use app::persistence::user::{create_user, get_user, search_users};
use app::state::AppState;
use models::params::user::CreateUserParams;
use models::queries::user::UserQuery;
use models::schemas::user::{UserListSchema, UserSchema};

use crate::error::ApiError;
use crate::extractor::{Json, Valid};

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

pub fn create_user_router() -> Router<AppState> {
    Router::new()
        .route("/", post(users_post).get(users_get))
        .route("/:id", get(users_id_get))
}
