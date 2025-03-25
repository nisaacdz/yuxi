use super::{middleware::auth, UserSession};
use api::{setup_config, setup_db, setup_router};
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tracing::info;
use utils::{create_dev_db, migrate};

fn on_connect(socket: SocketRef, Data(data): Data<Value>) {
    // Middleware should have set the user session so we can unwrap safely
    let user = socket.req_parts().extensions.get::<UserSession>().unwrap();
    // 3 things will all be relatively constant: client_id, user_id, and socket.id
    info!(
        "Socket.IO connected: {:?} {:?} {:?}",
        user.client_id, user.user_id, socket.id
    );
    socket.emit("auth", &data).ok();

    socket.on("message", async |socket: SocketRef, Data::<Value>(data)| {
        info!("Received event: {:?}", data);
        socket.to("333").emit("message-back", &data).await.ok();
    });
}

async fn worker(child_num: u32, db_url: &str, prefork: bool, listener: std::net::TcpListener) {
    tracing::info!("Worker {} started", child_num);

    let conn = setup_db(db_url, prefork).await;

    if child_num == 0 {
        migrate(&conn).await.expect("Migration failed!");
    }

    let (socket_layer, io) = SocketIo::new_layer();

    io.ns("/", on_connect);

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

    let listener = std::net::TcpListener::bind(config.get_server_url()).expect("bind to port");
    listener.set_nonblocking(true).expect("non blocking failed");
    tracing::debug!("listening on http://{}", listener.local_addr().unwrap());

    run_non_prefork(&config.db_url, listener);
}
