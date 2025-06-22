use axum::{Router, extract::State, routing::get};
use sea_orm::{ConnectionTrait, Statement};

use app::state::AppState;

use crate::error::ApiError;

async fn root_get(state: State<AppState>) -> Result<String, ApiError> {
    Ok("Hello, World from DB!".into())
}

pub fn create_root_router() -> Router<AppState> {
    Router::new().route("/", get(root_get))
}
