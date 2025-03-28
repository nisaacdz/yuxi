use auth::create_auth_router;
use axum::Router;

pub mod auth;
pub mod root;
pub mod tournament;
pub mod user;

use app::state::AppState;
use root::create_root_router;
use tournament::create_tournament_router;
use user::create_user_router;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(create_auth_router())
        .merge(create_root_router())
        .merge(create_user_router())
        .merge(create_tournament_router())
        .with_state(state)
}
