use app::{state::AppState, utils::decode_data};
use axum::{
    extract::{Request, State},
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use models::schemas::user::ClientSchema;
use uuid::Uuid;

use crate::error::ApiError;

pub async fn client_extension(
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

    let client_session = token
        .map(|token| decode_data::<ClientSchema>(&state.config, token).ok())
        .flatten();

    let client_session = client_session.or_else(|| {
        let x_client_id = headers
            .get("X-Client-ID")
            .and_then(|header| header.to_str().ok())
            .map(|v| Uuid::parse_str(v).ok())
            .flatten();

        x_client_id.map(|client_id| ClientSchema::from_id(client_id.to_string()))
    });

    let client_session = client_session.unwrap_or_else(|| ClientSchema {
        id: Uuid::new_v4().to_string(),
        user: None,
        updated: Utc::now(),
    });

    let client_id = &client_session.id;

    let header_value = HeaderValue::from_str(client_id).unwrap();

    req.extensions_mut().insert(client_session);

    let mut res = next.run(req).await;

    res.headers_mut()
        .insert(HeaderName::from_static("X-Client-ID"), header_value);

    Ok(res)
}
