use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method, header};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use app::config::Config;
use app::state::AppState;
use socketioxide::SocketIo;
use tower_cookies::cookie::time::Duration;
use tower_http::cors::CorsLayer;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

use crate::action::on_connect;
use crate::middleware::session;
use crate::routers::create_router;

pub fn setup_router(config: Config, conn: DatabaseConnection) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::OPTIONS,
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static("x-client"),
        ])
        .allow_origin(
            config
                .allowed_origin
                .parse::<HeaderValue>()
                .expect("Failed to parse allowed origin"),
        )
        .allow_credentials(true);

    let session_store = MemoryStore::default();

    let session_layer = SessionManagerLayer::new(session_store)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(86400 * 7)));

    let (socket_layer, io) = SocketIo::new_layer();

    {
        let conn = conn.clone();
        io.ns("/tournament", async move |socket, data| {
            on_connect(conn, socket, data).await;
        });
    }
    create_router(AppState { conn })
        .layer(cors)
        .layer(session_layer)
        .layer(axum::middleware::from_fn(session::client_session))
        .layer(socket_layer)
}

pub fn setup_config() -> Config {
    dotenvy::dotenv().ok();
    Config::from_env()
}

pub async fn setup_db(db_url: &str, prefork: bool) -> DatabaseConnection {
    let mut opt = ConnectOptions::new(db_url);
    opt.max_lifetime(std::time::Duration::from_secs(60));

    if !prefork {
        opt.min_connections(10).max_connections(100);
    }

    Database::connect(opt)
        .await
        .expect("Database connection failed")
}
