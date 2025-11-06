use axum::{Router, extract::State, routing::get};
use sea_orm::{ConnectionTrait, Statement};

use app::state::AppState;

use crate::error::ApiError;

async fn root_get(state: State<AppState>) -> Result<String, ApiError> {
    let result = state
        .conn
        .query_one(Statement::from_string(
            state.conn.get_database_backend(),
            "SELECT 'Hello, World from DB!'",
        ))
        .await
        .map_err(ApiError::from)?;

    result
        .ok_or_else(|| ApiError::from(sea_orm::DbErr::RecordNotFound("Query result not found".to_string())))?
        .try_get_by(0)
        .map_err(|e| e.into())
}

pub fn create_root_router() -> Router<AppState> {
    Router::new().route("/", get(root_get))
}
