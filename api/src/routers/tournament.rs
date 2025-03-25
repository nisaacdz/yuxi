use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use sea_orm::TryIntoModel;

use app::persistence::tournaments::{create_tournament, search_tournaments};
use app::state::AppState;
use models::params::tournament::CreateTournamentParams;
use models::queries::tournament::TournamentQuery;
use models::schemas::tournament::{TournamentListSchema, TournamentSchema};

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
    Query(query): Query<Option<TournamentQuery>>,
) -> Result<impl IntoResponse, ApiError> {
    let query = query.unwrap_or_default();

    let tournaments = search_tournaments(&state.conn, query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(TournamentListSchema::from(tournaments)))
}

pub fn create_tournament_router() -> Router<AppState> {
    Router::new().route("/", get(tournaments_get).post(tournaments_post))
}
