use auth::create_auth_router;
use axum::Router;

pub mod auth;
pub mod root;
pub mod tournament;
pub mod user;

use app::state::AppState;
use tournament::create_tournament_router;
use user::create_user_router;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/auth", create_auth_router())
        .nest("/users", create_user_router())
        .nest("/tournaments", create_tournament_router())
        .with_state(state)
}
