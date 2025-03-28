use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};

use app::error::UserError;
use app::persistence::users::get_user;
use app::state::AppState;
use models::schemas::user::UserSchema;
use models::schemas::user::UserSession;

use crate::error::ApiError;
use crate::extractor::Json;

#[axum::debug_handler]
pub async fn login_post(
    state: State<AppState>,
    mut req: Request,
) -> Result<impl IntoResponse, ApiError> {
    // pretend to login
    let user = get_user(&state.conn, 1).await.map_err(ApiError::from)?;
    let user = user.unwrap();
    req.extensions_mut().insert(UserSession {
        client_id: "123".to_string(),
        user: Some(UserSchema::from(user)),
    });
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn register_post(
    state: State<AppState>,
    mut req: Request,
) -> Result<impl IntoResponse, ApiError> {
    // pretend to login
    let user = get_user(&state.conn, 1).await.map_err(ApiError::from)?;
    let user = user.unwrap();
    req.extensions_mut().insert(UserSession {
        client_id: "123".to_string(),
        user: Some(UserSchema::from(user)),
    });
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn logout_post(mut req: Request) -> Result<impl IntoResponse, ApiError> {
    let _user_session = req.extensions_mut().remove::<UserSession>();
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
pub async fn me_get(state: State<AppState>, req: Request) -> Result<impl IntoResponse, ApiError> {
    let user = if let Some(user) = &req.extensions().get::<UserSession>().unwrap().user {
        get_user(&state.conn, user.id)
            .await
            .map_err(ApiError::from)?
    } else {
        None
    };
    user.map(|user| Json(UserSchema::from(user)))
        .ok_or_else(|| UserError::NotFound.into())
}

pub fn create_auth_router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login_post))
        .route("/auth/logout", post(logout_post))
        .route("/auth/register", post(register_post))
        .route("/auth/me", get(me_get))
}
