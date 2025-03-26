use crate::{
    cache::{redis_delete_typing_session, redis_get_typing_session, redis_set_typing_session},
    TypingSession,
};

use super::UserSession;
use app::persistence::tournaments::get_tournament;
use chrono::{TimeDelta, Utc};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;
use socketioxide::extract::{Data, SocketRef};
use tracing::info;

mod typing_api;

// time left for scheduled tournament after which no one can join in seconds

#[derive(Deserialize, Clone, Debug)]
struct JoinArgs {
    tournament_id: String,
}

pub async fn on_connect(conn: DatabaseConnection, socket: SocketRef, Data(data): Data<Value>) {
    // Middleware should have set the user session so we can unwrap safely
    let user = socket
        .req_parts()
        .extensions
        .get::<UserSession>()
        .unwrap()
        .clone();
    // 3 things will all be relatively constant: client_id, user_id, and socket.id
    info!(
        "Socket.IO connected: {:?} {:?} {:?}",
        user.client_id, user.user_id, socket.id
    );

    socket.on(
        "join",
        async move |socket: SocketRef, Data::<JoinArgs>(JoinArgs { tournament_id })| {
            typing_api::handle_join(tournament_id, user.clone(), socket, conn.clone()).await;
        },
    );

    socket.on("disconnect", async move |socket: SocketRef| {
        info!("Socket.IO disconnected: {:?}", socket.id);
    });
}
