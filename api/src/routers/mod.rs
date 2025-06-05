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
    let routes = Router::new()
        .nest("/auth", create_auth_router())
        .nest("/users", create_user_router())
        .nest("/tournaments", create_tournament_router());

    Router::new()
        .merge(create_root_router())
        .nest("/api/v1", routes)
        .with_state(state)
}
