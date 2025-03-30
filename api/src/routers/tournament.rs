use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use sea_orm::TryIntoModel;

use app::persistence::tournaments::{create_tournament, search_tournaments};
use app::state::AppState;
use models::params::tournament::CreateTournamentParams;
use models::queries::PaginationQuery;
use models::schemas::tournament::TournamentSchema;

use crate::error::ApiError;
use crate::extractor::{Json, Valid};

async fn tournaments_post(
    state: State<AppState>,
    Valid(Json(params)): Valid<Json<CreateTournamentParams>>,
) -> Result<impl IntoResponse, ApiError> {
    let tournament = create_tournament(&state.conn, params)
        .await
        .map_err(ApiError::from)?;

    let tournament = tournament.try_into_model().unwrap();
    Ok((
        StatusCode::CREATED,
        Json(TournamentSchema::from(tournament)),
    ))
}

#[axum::debug_handler]
async fn tournaments_get(
    state: State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = search_tournaments(&state.conn, query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub fn create_tournament_router() -> Router<AppState> {
    Router::new()
        .route("/tournaments", get(tournaments_get))
        .route("/tournaments/new", post(tournaments_post))
}
