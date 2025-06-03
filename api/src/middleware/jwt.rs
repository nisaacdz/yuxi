use app::{state::AppState, utils::decode_data};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use tracing;
use uuid::Uuid;

use crate::error::ApiError;

pub async fn jwt_auth(
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

    let client_session = match token {
        Some(token) => {
            match decode_data::<ClientSchema>(&state.config, token) {
                Ok(client) => {
                    tracing::trace!("JWT token decoded successfully for client: {}", client.id);
                    client
                }
                Err(e) => {
                    tracing::debug!("Invalid JWT token: {:?}", e);
                    // Create anonymous session for invalid/expired tokens
                    ClientSchema {
                        id: Uuid::new_v4().to_string(),
                        user: None,
                        updated: Utc::now(),
                    }
                }
            }
        }
        None => {
            tracing::trace!("No JWT token found, creating anonymous session");
            // Create anonymous session when no token provided
            ClientSchema {
                id: Uuid::new_v4().to_string(),
                user: None,
                updated: Utc::now(),
            }
        }
    };

    req.extensions_mut().insert(client_session);

    Ok(next.run(req).await)
}
