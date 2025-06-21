use app::{state::AppState, utils::decode_data};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use models::schemas::user::{AuthSchema, UserSchema};

use crate::error::ApiError;

pub async fn extension(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let headers = req.headers();

    // Try to extract JWT from Authorization header
    let token = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| {
            if header.starts_with("Bearer ") {
                Some(&header[7..])
            } else {
                None
            }
        });

    let auth_user = token
        .map(|token| decode_data::<UserSchema>(&state.config, token).ok())
        .flatten();

    req.extensions_mut().insert(AuthSchema { user: auth_user });

    Ok(next.run(req).await)
}
