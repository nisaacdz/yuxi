use api::{setup_config, setup_db, setup_router};

pub async fn run() {
    let config = setup_config().await;
    let listener = std::net::TcpListener::bind(config.get_server_url()).expect("bind to port");
    listener.set_nonblocking(true).expect("non blocking failed");
    tracing::debug!("listening on http://{}", listener.local_addr().unwrap());

    tracing::info!("Worker started");

    let conn = setup_db(&config.db_url).await;

    utils::migrate(&conn).await.expect("Migration failed!");

    let router = setup_router(config, conn);
    let listener = tokio::net::TcpListener::from_std(listener).expect("bind to port");
    axum::serve(listener, router).await.expect("start server");
}
