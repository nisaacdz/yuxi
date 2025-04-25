use std::{sync::Arc, time::Duration};

use models::schemas::user::ClientSchema;
use moderation::FrequencyMonitor;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use socketioxide::{
    SocketIo,
    extract::{Data, SocketRef},
};
use timeout::TimeoutMonitor;
use tracing::info;

pub(self) mod handlers;
pub(self) mod logic;
mod moderation;
pub(self) mod state;
mod manager;
mod registry;

mod timeout;

#[derive(Deserialize, Clone, Debug)]
struct TypeArgs {
    character: char,
}

pub async fn enter_tournament(conn: DatabaseConnection, io: SocketIo, socket: SocketRef) {
    let tournament_id = socket.ns().trim_start_matches("/tournament/").to_string();
    let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
    info!(
        "Socket.IO connected to dynamic namespace {} : {:?}",
        tournament_id, client
    );

    handlers::handle_join(
        tournament_id.to_owned(),
        io.clone(),
        socket.clone(),
        conn.clone(),
    )
    .await;

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
        let io = io.clone();
        let timeout_monitor = {
            let socket = socket.clone();

            let after_timeout_fn = { async move || info!("Timedout user now typing") };

            Arc::new(TimeoutMonitor::new(
                async move || {
                    handlers::handle_timeout(&client, socket).await;
                },
                after_timeout_fn,
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
            let io = io.clone();
            async move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
                let processor = async move {
                    frequency_monitor
                        .call(character, move |chars: Vec<char>| {
                            handlers::handle_typing(io, socket, chars)
                        })
                        .await;
                };

                timeout_monitor.call(processor).await;
            }
        });
    }

    {
        let io = io.clone();
        socket.on("leave-tournament", async move |socket: SocketRef| {
            handlers::handle_leave(io, socket, tournament_id).await;
        });
    }

    {
        socket.on("disconnect", async move |socket: SocketRef| {
            info!("Socket.IO disconnected: {:?}", socket.id);
            socket.leave_all();
        });
    }
}
