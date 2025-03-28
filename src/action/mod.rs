use models::schemas::user::UserSession;
use moderation::TypingModerator;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;
use socketioxide::extract::{Data, SocketRef};
use tracing::info;

mod moderation;
mod typing_api;

#[derive(Deserialize, Clone, Debug)]
struct JoinArgs {
    tournament_id: String,
}

#[derive(Deserialize, Clone, Debug)]
struct TypeArgs {
    character: char,
}

pub async fn on_connect(conn: DatabaseConnection, socket: SocketRef, Data(_data): Data<Value>) {
    let user = socket
        .req_parts()
        .extensions
        .get::<UserSession>()
        .unwrap()
        .clone();
    info!(
        "Socket.IO connected: {:?} {:?} {:?}",
        user.client_id,
        user.user.as_ref().map(|u| u.id),
        socket.id
    );

    socket.on(
        "join-tournament",
        async move |socket: SocketRef, Data::<JoinArgs>(JoinArgs { tournament_id })| {
            typing_api::handle_join(tournament_id, socket, conn.clone()).await;
        },
    );

    socket.on(
        "type-character",
        async move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
            TypingModerator(move |chars| typing_api::handle_typing(socket, chars))
                .moderate(&user.client_id, character)
                .await;
        },
    );

    socket.on(
        "leave-tournament",
        async move |socket: SocketRef, Data::<JoinArgs>(JoinArgs { tournament_id })| {
            typing_api::handle_leave(tournament_id, socket).await;
        },
    );

    socket.on("disconnect", async move |socket: SocketRef| {
        info!("Socket.IO disconnected: {:?}", socket.id);
    });
}
