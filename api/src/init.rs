use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method, header};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use app::config::Config;
use app::state::AppState;
use socketioxide::SocketIo;
use tower_cookies::Key;
use tower_cookies::cookie::time::Duration;
use tower_http::cors::CorsLayer;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

use crate::action::enter_tournament;
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
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(86400 * 7)))
        .with_signed(Key::from(config.session_secret.as_bytes()));

    let (socket_layer, io) = SocketIo::new_layer();

    {
        let conn = conn.clone();
        let res = io.dyn_ns("/tournament/{tournament_id}", async move |socket, io| {
            enter_tournament(conn, socket, io).await;
        });

        match res {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Failed to setup dynamic namespace: {e}");
                panic!("Failed to setup dynamic namespace: {e}")
            }
        }
    }

    let app_state = AppState { conn };

    create_router(app_state)
        .layer(socket_layer)
        .layer(axum::middleware::from_fn(session::client_session))
        .layer(session_layer)
        .layer(cors)
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
