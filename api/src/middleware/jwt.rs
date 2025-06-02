use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use tracing;
use uuid::Uuid;

use crate::{error::ApiError, utils::jwt::JwtService};

pub async fn jwt_auth(
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
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
            match jwt_service.verify_token(token) {
                Ok(claims) => {
                    tracing::trace!(
                        "JWT token verified successfully for client: {}",
                        claims.client_id
                    );
                    ClientSchema {
                        id: claims.client_id,
                        user: claims.user_id.map(|id| models::schemas::user::UserSchema {
                            id,
                            username: "".to_string(), // We'll need to fetch this from DB if needed
                            email: "".to_string(),    // We'll need to fetch this from DB if needed
                        }),
                        updated: Utc::now(),
                    }
                }
                Err(e) => {
                    tracing::debug!("Invalid JWT token: {}", e);
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
