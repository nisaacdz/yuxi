use axum::{extract::Request, middleware::Next, response::Response};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use tracing;
use uuid::Uuid;

use crate::{error::ApiError, utils::jwt::JwtService};

pub async fn jwt_auth(mut req: Request, next: Next) -> Result<Response, ApiError> {
    let headers = req.headers();
    let jwt_service = JwtService::new()?;

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
            match jwt_service.decode_client(token) {
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
