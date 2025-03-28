use super::middleware::auth;
use crate::{action::on_connect, cache::initialize_redis};
use api::{setup_config, setup_db, setup_router};
use app::config::Config;
use axum::http::Method;
use socketioxide::SocketIo;
use tower_http::cors::{Any, CorsLayer};
use utils::{create_dev_db, migrate};

async fn worker(child_num: u32, config: Config, prefork: bool, listener: std::net::TcpListener) {
    tracing::info!("Worker {} started", child_num);

    let conn = setup_db(&config.db_url, prefork).await;

    if child_num == 0 {
        migrate(&conn).await.expect("Migration failed!");
    }

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    let (socket_layer, io) = SocketIo::new_layer();

    {
        let conn = conn.clone();
        io.ns("/", async move |socket, data| {
            on_connect(conn, socket, data).await;
        });
    }

    // Set up the router with authentication middleware
    let router = setup_router(conn)
        .layer(cors)
        .layer(axum::middleware::from_fn(auth::auth))
        .layer(socket_layer);

    let listener = tokio::net::TcpListener::from_std(listener).expect("bind to port");
    axum::serve(listener, router).await.expect("start server");
}

fn run_non_prefork(config: Config, listener: std::net::TcpListener) {
    create_dev_db(&config.db_url);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(worker(0, config, false, listener));
}

pub fn run() {
    let config = setup_config();
    initialize_redis(&config.redis_url);
    let listener = std::net::TcpListener::bind(config.get_server_url()).expect("bind to port");
    listener.set_nonblocking(true).expect("non blocking failed");
    tracing::debug!("listening on http://{}", listener.local_addr().unwrap());

    run_non_prefork(config, listener);
}
