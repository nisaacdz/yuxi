use axum::{extract::Request, middleware::Next, response::Response};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use tower_sessions::Session;
use tracing;
use uuid::Uuid;

use crate::error::ApiError;

pub const CLIENT_SESSION_KEY: &str = "client_session_data_v1";

pub async fn client_session(
    session: Session,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let loaded_session_result = session.get::<ClientSchema>(CLIENT_SESSION_KEY).await;
    let client_session = match loaded_session_result {
        Ok(Some(state)) => {
            tracing::trace!("Loaded existing client session.");
            Ok(state)
        }
        Ok(None) => {
            tracing::info!("No client session found, creating new one.");

            let new_state = ClientSchema {
                client_id: Uuid::new_v4().to_string(),
                user: None,
                updated: Utc::now(),
            };

            session
                .insert(CLIENT_SESSION_KEY, &new_state)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to insert new client session data: {}", e);
                    ApiError(anyhow::anyhow!("Failed to save session state").context(e))
                })?;

            Ok(new_state)
        }
        Err(e) => {
            tracing::error!("Failed to load client session data from store: {}", e);
            Err(anyhow::anyhow!("Failed to load session state").context(e))
        }
    }?;

    req.extensions_mut().insert(client_session);

    Ok(next.run(req).await)
}
