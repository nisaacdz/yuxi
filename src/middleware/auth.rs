use axum::http::HeaderMap;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use models::schemas::user::UserSession;
use std::convert::Infallible;
use std::task::{Context, Poll};
use tower_service::Service;
use uuid::Uuid;

pub async fn auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    if req.extensions().get::<UserSession>().is_some() {
        Ok(next.run(req).await)
    } else {
        // Check for client_id in cookies
        let headers = req.headers();
        let user_session = UserSession {
            client_id: get_or_set_client_id(headers),
            user: None,
        };

        req.extensions_mut().insert(user_session);

        Ok(next.run(req).await)
    }
}

fn get_or_set_client_id(headers: &HeaderMap) -> String {
    if let Some(cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = cookie.to_str() {
            for part in cookie_str.split(';') {
                let part = part.trim();
                if part.starts_with("client_id=") {
                    return part["client_id=".len()..].to_string();
                }
            }
        }
    }

    // Generate a new client_id if not found
    let client_id = Uuid::new_v4().to_string();
    client_id
}

pub struct AuthMiddleware<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for AuthMiddleware<S>
where
    S: Service<Request<B>, Response = Response, Error = Infallible> + Clone,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if req.extensions().get::<UserSession>().is_some() {
            self.inner.call(req)
        } else {
            // Check for client_id in cookies
            let headers = req.headers();
            let user_session = UserSession {
                client_id: get_or_set_client_id(headers),
                user: None,
            };

            req.extensions_mut().insert(user_session).unwrap();

            self.inner.call(req)
        }
    }
}
