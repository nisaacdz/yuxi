use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method, header};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::action::register_tournament_namespace;
use crate::middleware::extension::extension;
use crate::routers::create_router;
use app::cache::{TournamentRegistry, TypingSessionRegistry};
use app::config::Config;
use app::state::AppState;
use socketioxide::SocketIo;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

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
            HeaderName::from_static("x-noauth-unique"),
        ])
        .allow_origin(
            config
                .allowed_origin
                .parse::<HeaderValue>()
                .expect("Failed to parse allowed origin"),
        )
        .allow_credentials(true);

    let tournament_registry = TournamentRegistry::new();
    let typing_session_registry = TypingSessionRegistry::new();
    let (socket_layer, socket_io) = SocketIo::new_layer();

    let app_state = AppState {
        conn,
        config,
        tournament_registry,
        typing_session_registry,
        socket_io,
    };

    {
        let app_state = app_state.clone();

        register_tournament_namespace(app_state);
    }

    create_router(app_state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(socket_layer)
        .layer(axum::middleware::from_fn_with_state(
            app_state,
            extension,
        ))
        .layer(cors)
}

pub fn setup_config() -> Config {
    dotenvy::dotenv().ok();
    Config::from_env()
}

pub async fn setup_db(db_url: &str) -> DatabaseConnection {
    let mut opt = ConnectOptions::new(db_url);
    opt.max_lifetime(std::time::Duration::from_secs(60));

    opt.min_connections(10).max_connections(100);

    Database::connect(opt)
        .await
        .expect("Database connection failed")
}
