use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method, header};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::action::registry::register_tournament_namespace;
use crate::cache::{TournamentRegistry, TypingSessionRegistry};
use crate::middleware::jwt::{self, jwt_auth};
use crate::routers::create_router;
use app::config::Config;
use app::state::AppState;
use socketioxide::SocketIo;
use socketioxide::extract::SocketRef;
use tower_cookies::Key;
use tower_cookies::cookie::time::Duration;
use tower_http::cors::CorsLayer;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

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

    let (socket_layer, io) = SocketIo::new_layer();

    {
        let conn = conn.clone();
        io.ns("/", async move |socket: SocketRef| {
            tracing::info!("default namespace reached: {}", socket.id)
        });
        let tournament_registry = TournamentRegistry::new();
        let typing_sessions = TypingSessionRegistry::new();

        register_tournament_namespace(io, conn, tournament_registry, typing_sessions);
    }

    let app_state = AppState { conn };

    create_router(app_state)
        .layer(socket_layer)
        .layer(axum::middleware::from_fn(jwt_auth))
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
