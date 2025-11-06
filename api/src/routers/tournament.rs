use anyhow::anyhow;
use axum::{
    Extension, Router,
    extract::{Path, Query, Request, State},
    response::IntoResponse,
    routing::{get, post},
};

use app::persistence::tournaments::{create_tournament, get_tournament, search_tournaments};
use app::state::AppState;
use models::params::tournament::CreateTournamentParams;
use models::schemas::tournament::TournamentSchema;
use models::{queries::TournamentPaginationQuery, schemas::user::AuthSchema};

use crate::{ApiResponse, error::ApiError};
use crate::{
    decode_noauth,
    extractor::{Json, Valid},
};

async fn tournaments_post(
    state: State<AppState>,
    Extension(auth_state): Extension<AuthSchema>,
    Valid(Json(params)): Valid<Json<CreateTournamentParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user = match auth_state.user {
        Some(user) => user,
        None => return Err(ApiError::from(anyhow!("Unauthorized"))),
    };

    tracing::debug!("Creating tournament with params: {:?}", params);

    let tournament = create_tournament(&state.conn, params, &user)
        .await
        .map_err(ApiError::from)?;

    let result = ApiResponse::success(
        "Tournament created successfully",
        Some(TournamentSchema::from(tournament)),
    );

    Ok(Json(result))
}

#[axum::debug_handler]
async fn tournaments_get(
    state: State<AppState>,
    Extension(auth_state): Extension<AuthSchema>,
    Query(query): Query<TournamentPaginationQuery>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let noauth = request.headers().get("x-noauth-unique");
    let member_id = noauth.and_then(|value| decode_noauth(value.as_ref()));
    let member_id = member_id.as_ref().map(|v| v.as_ref());

    let result = search_tournaments(
        &state,
        query,
        auth_state.user.as_ref().map(|u| u.id.as_str()),
        member_id,
    )
    .await
    .map_err(ApiError::from)?;

    let response = ApiResponse::success("Tournaments retrieved Successfully", Some(result));

    Ok(Json(response))
}

#[axum::debug_handler]
async fn tournaments_id_get(
    state: State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let result = get_tournament(&state.conn, id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(anyhow!("Tournament not found")))?;

    let response = ApiResponse::success("Tournament retrieved Successfully", Some(result));

    Ok(Json(response))
}

pub fn create_tournament_router() -> Router<AppState> {
    Router::new()
        .route("/", get(tournaments_get))
        .route("/", post(tournaments_post))
        .route("/{id}", get(tournaments_id_get))
}
