use anyhow::anyhow;
use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use sea_orm::TryIntoModel;

use app::persistence::tournaments::{
    create_tournament, get_tournament, search_upcoming_tournaments,
};
use app::state::AppState;
use models::queries::PaginationQuery;
use models::schemas::tournament::TournamentSchema;
use models::{params::tournament::CreateTournamentParams, schemas::user::ClientSchema};

use crate::extractor::{Json, Valid};
use crate::{ApiResponse, error::ApiError};

async fn tournaments_post(
    state: State<AppState>,
    Extension(client): Extension<ClientSchema>,
    Valid(Json(params)): Valid<Json<CreateTournamentParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let user = match client.user {
        Some(user) => user,
        None => return Err(ApiError::from(anyhow!("Unauthorized"))),
    };

    let tournament = create_tournament(&state.conn, params, &user)
        .await
        .map_err(ApiError::from)?;

    let tournament = tournament.try_into_model().unwrap();

    let result = ApiResponse::success(
        "Tournament created successfully",
        Some(TournamentSchema::from(tournament)),
    );

    Ok(Json(result))
}

#[axum::debug_handler]
async fn tournaments_get(
    state: State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = search_upcoming_tournaments(&state.conn, query)
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
