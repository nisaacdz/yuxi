use std::sync::Arc;

use models::schemas::user::UserSession;
use moderation::FrequencyMonitor;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::Value;
use socketioxide::extract::{Data, SocketRef};
use timeout::TimeoutMonitor;
use tracing::info;

mod moderation;
mod timeout;
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
    {
        let timeout_monitor = Arc::new(TimeoutMonitor::new(async move || {}));
        let frequency_monitor = Arc::new(FrequencyMonitor::new());
        socket.on("type-character", {
            let frequency_monitor = frequency_monitor.clone();
            let timeout_monitor = timeout_monitor.clone();
            async move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
                let socket_clone_outer = socket.clone();

                let processor = async move {
                    let inner_closure = move |chars: Vec<char>| {
                        let socket_clone_for_call = socket_clone_outer.clone();
                        typing_api::handle_typing(socket_clone_for_call, chars)
                    };
                    frequency_monitor.call(character, inner_closure).await;
                };

                timeout_monitor.call(processor).await;
            }
        });
    }

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
