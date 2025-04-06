use std::{sync::Arc, time::Duration};

use models::schemas::user::ClientSchema;
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
    let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
    info!(
        "Socket.IO connected: {:?} {:?} {:?}",
        client.id,
        client.user.as_ref().map(|u| u.id),
        socket.id
    );

    socket.on(
        "join-tournament",
        async move |socket: SocketRef, Data::<JoinArgs>(JoinArgs { tournament_id })| {
            typing_api::handle_join(tournament_id, socket, conn.clone()).await;
        },
    );
    {
        // wait period before processing a new character
        let debounce_duration = Duration::from_millis(100);
        // user should only experience at worst 3s lag time
        // but will likely be in millis under normal circumstances
        let max_process_wait = Duration::from_secs(1);
        // processing shouldn't lag behind by more than 15 chars from current position
        // but will likely be instantaneous under normal circumstances
        let max_process_stack_size = 15;
        let cleanup_wait_duration = Duration::from_secs(30);
        let client = client.clone();
        let timeout_monitor = {
            let socket = socket.clone();
            Arc::new(TimeoutMonitor::new(
                async move || {
                    typing_api::handle_timeout(&client, socket).await;
                },
                async move || {},
                cleanup_wait_duration,
            ))
        };

        let frequency_monitor = Arc::new(FrequencyMonitor::new(
            debounce_duration,
            max_process_wait,
            max_process_stack_size,
        ));

        socket.on("type-character", {
            let frequency_monitor = frequency_monitor.clone();
            let timeout_monitor = timeout_monitor.clone();
            async move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
                let processor = async move {
                    frequency_monitor
                        .call(character, move |chars: Vec<char>| {
                            typing_api::handle_typing(socket, chars)
                        })
                        .await;
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
