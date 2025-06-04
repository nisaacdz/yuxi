use anyhow::anyhow;
use axum::{
    Extension, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use sea_orm::TryIntoModel;

use app::persistence::tournaments::{create_tournament, search_upcoming_tournaments};
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
    match client.user {
        Some(user) => {
            let tournament = create_tournament(&state.conn, params, &user)
                .await
                .map_err(ApiError::from)?;

            let tournament = tournament.try_into_model().unwrap();
            Ok((
                StatusCode::CREATED,
                Json(TournamentSchema::from(tournament)),
            ))
        }
        None => Err(ApiError::from(anyhow!("Unauthorized"))),
    }
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

pub fn create_tournament_router() -> Router<AppState> {
    Router::new()
        .route("/", get(tournaments_get))
        .route("/new", post(tournaments_post))
}
