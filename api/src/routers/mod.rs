use axum::Router;

pub mod root;
pub mod tournament;
pub mod user;

use app::state::AppState;
use root::create_root_router;
use tournament::create_tournament_router;
use user::create_user_router;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/users", create_user_router())
        .nest("/tournaments", create_tournament_router())
        .nest("/", create_root_router())
        .with_state(state)
}
