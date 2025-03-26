use super::middleware::auth;
use crate::{action::on_connect, cache::initialize_redis};
use api::{setup_config, setup_db, setup_router};
use socketioxide::SocketIo;
use utils::{create_dev_db, migrate};

async fn worker(child_num: u32, db_url: &str, prefork: bool, listener: std::net::TcpListener) {
    tracing::info!("Worker {} started", child_num);

    let conn = setup_db(db_url, prefork).await;

    if child_num == 0 {
        migrate(&conn).await.expect("Migration failed!");
    }

    let (socket_layer, io) = SocketIo::new_layer();

    {
        let conn = conn.clone();
        io.ns("/", async move |socket, data| {
            on_connect(conn, socket, data).await;
        });
    }

    // Set up the router with authentication middleware
    let router = setup_router(conn)
        .route_layer(axum::middleware::from_fn(auth::auth))
        .layer(socket_layer);

    let listener = tokio::net::TcpListener::from_std(listener).expect("bind to port");
    axum::serve(listener, router).await.expect("start server");
}

fn run_non_prefork(db_url: &str, listener: std::net::TcpListener) {
    create_dev_db(db_url);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(worker(0, db_url, false, listener));
}

pub fn run() {
    let config = setup_config();
    initialize_redis(&config.redis_url);
    let listener = std::net::TcpListener::bind(config.get_server_url()).expect("bind to port");
    listener.set_nonblocking(true).expect("non blocking failed");
    tracing::debug!("listening on http://{}", listener.local_addr().unwrap());

    run_non_prefork(&config.db_url, listener);
}
