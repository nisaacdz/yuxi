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

#[utoipa::path(
    post,
    path = "",
    request_body = CreateTournamentParams,
    responses(
        (status = 201, description = "Tournament created", body = TournamentSchema),
        (status = 400, description = "Bad request", body = ApiErrorResponse),
        (status = 422, description = "Validation error", body = ParamsErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    )
)]
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

#[utoipa::path(
    get,
    path = "",
    params(
        TournamentQuery
    ),
    responses(
        (status = 200, description = "List Tournaments", body = TournamentListSchema),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    )
)]
async fn tournaments_get(
    state: State<AppState>,
    query: Option<Query<TournamentQuery>>,
) -> Result<impl IntoResponse, ApiError> {
    let Query(query) = query.unwrap_or_default();

    let tournaments = search_tournaments(&state.conn, query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(TournamentListSchema::from(tournaments)))
}

pub fn create_tournament_router() -> Router<AppState> {
    Router::new().route("/", get(tournaments_get).post(tournaments_post))
}
