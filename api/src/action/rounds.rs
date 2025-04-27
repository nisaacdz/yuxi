use std::{collections::HashMap, sync::Arc, time::Duration};
use anyhow::Result;
use chrono::Utc;
use models::schemas::{tournament::TournamentSession, typing::{TournamentUpdateSchema, TypingSessionSchema}, user::ClientSchema};
use sea_orm::DatabaseConnection;
use socketioxide::{extract::{Data, SocketRef}, SocketIo};
use tracing::{error, info, warn};

use crate::{action::{handlers::{handle_timeout, try_join_tournament, handle_typing, handle_leave}, moderation::FrequencyMonitor, TypeArgs}, cache::Cache};

use crate::action::{state::ApiResponse, timeout::TimeoutMonitor};

const JOIN_DEADLINE_SECONDS: i64 = 15;
const INACTIVITY_TIMEOUT_DURATION: Duration = Duration::from_secs(30);
const DEBOUNCE_DURATION: Duration = Duration::from_millis(100);
const MAX_PROCESS_WAIT: Duration = Duration::from_secs(1);
const MAX_PROCESS_STACK_SIZE: u32 = 15;

pub struct TournamentManager {
    tournament_id: String,
    tournament_state: Arc<tokio::sync::Mutex<TournamentSession>>,
    participants: Arc<tokio::sync::Mutex<HashMap<String, TypingSessionSchema>>>,
    io: SocketIo,
    conn: DatabaseConnection,
    cache: Cache<TypingSessionSchema>,
    typing_text: Arc<String>,
}

impl TournamentManager {
    pub async fn init(
        tournament_id: &str,
        conn: DatabaseConnection,
        io: SocketIo,
        cache: Cache<TypingSessionSchema>
    ) -> Result<Self> {
        info!("Initializing TournamentManager for {}", tournament_id);
        let tournament_schema = app::persistence::tournaments::get_tournament(&conn, tournament_id.to_string())
            .await
            .map_err(|db_err| {
                error!("DB error fetching tournament {}: {}", tournament_id, db_err);
                anyhow::anyhow!("Failed to retrieve tournament details from DB")
            })?
            .ok_or_else(|| {
                warn!("Tournament {} not found in database", tournament_id);
                anyhow::anyhow!("Tournament not found")
            })?;

        let typing_text = app::persistence::text::get_or_generate_text(&conn, tournament_id).await?;

        let initial_session = TournamentSession::new(
            tournament_schema.id.clone(),
            tournament_schema.scheduled_for,
            Some(typing_text.clone())
        );

        let tournament_state = Arc::new(tokio::sync::Mutex::new(initial_session));

        let typing_text = Arc::new(typing_text);

        let participants = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        {
            let task_tournament_id = tournament_id.to_string();
            let tournament_state = tournament_state.clone();
            let task_scheduled_for = tournament_schema.scheduled_for;

            let task = async move {
                info!("Scheduled task running for tournament {}", task_tournament_id);
                let mut tlck = tournament_state.lock().await;
                tlck.started_at = Some(Utc::now());
            };
            
            match crate::scheduler::schedule_new_task(
                tournament_id.to_string(),
                task,
                task_scheduled_for,
            )
            .await
            {
                Ok(handle) => {
                    info!("Successfully scheduled start task for tournament {}", tournament_id);
                    Some(handle)
                }
                Err(schedule_err) => {
                    error!("Failed to schedule task for tournament {}: {}", tournament_id, schedule_err);
                    return Err(anyhow::anyhow!(
                        "Failed to schedule tournament start task: {}",
                        schedule_err
                    ));
                }
            }
        };

        Ok(Self {
            tournament_id: tournament_id.to_string(),
            tournament_state,
            participants,
            io,
            conn,
            typing_text,
            cache,
        })
    }

    
    pub async fn connect(
        self: &Arc<Self>,
        socket: SocketRef,
    ) -> Result<()> {
        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
        let tournament_id = &self.tournament_id;
        let client_id = client.id.clone();
        info!("Handling connection for client {} to tournament {}", client_id, tournament_id);

        match try_join_tournament(&tournament_id, self.io.clone(), socket.clone(), self.conn.clone(), self.cache.clone()).await {
            Ok(_) => {
                info!("Successfully joined tournament");
                socket.join(tournament_id.to_owned())
            }
            Err(err) => {
                error!("Failed to join tournament: {}", err);
                let _ = socket.emit("join:response", &ApiResponse::<()>::error("Failed to join tournament"));
                return Err(err);
            }
        }


        self.broadcast_tournament_update().await;


        self.register_socket_listeners(client.clone(), socket.clone());

        Ok(())
    }


    fn register_socket_listeners(self: &Arc<Self>, client: ClientSchema, socket: SocketRef) {
        let tournament_id = &self.tournament_id;
        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
        info!(
            "Socket.IO connected to dynamic namespace {} : {:?}",
            tournament_id, client
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
            let typing_text = self.typing_text.clone();
            let client = client.clone();
            let io = self.io.clone();
            let timeout_monitor = {
                let socket = socket.clone();

                let after_timeout_fn = { async move || info!("Timedout user now typing") };

                Arc::new(TimeoutMonitor::new(
                    async move || {
                        handle_timeout(&client, socket).await;
                    },
                    after_timeout_fn,
                    cleanup_wait_duration,
                ))
            };

            let cache = self.cache.clone();

            let frequency_monitor = Arc::new(FrequencyMonitor::new(
                debounce_duration,
                max_process_wait,
                max_process_stack_size,
            ));

            socket.on("type-character", {
                let frequency_monitor = frequency_monitor.clone();
                let timeout_monitor = timeout_monitor.clone();
                let io = io.clone();
                let typing_text = typing_text.clone();
                async move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
                    let processor = async move {
                        frequency_monitor
                            .call(character, move |chars: Vec<char>| {
                                handle_typing(io, socket, chars, cache, typing_text)
                            })
                            .await;
                    };

                    timeout_monitor.call(processor).await;
                }
            });
        }

        {
            let io = self.io.clone();
            socket.on("leave-tournament", async move |socket: SocketRef| {
                handle_leave(io, socket, tournament_id.to_owned()).await;
            });
        }

        {
            socket.on("disconnect", async move |socket: SocketRef| {
                info!("Socket.IO disconnected: {:?}", socket.id);
                socket.leave_all();
            });
        }
    }

    /// Internal logic for handling leave or disconnect.
    async fn handle_leave_internal(self: &Arc<Self>, client_id: &str, socket: &SocketRef, is_disconnect: bool) -> Result<()> {
         info!("Handling leave/disconnect for client {}. Is disconnect: {}", client_id, is_disconnect);
         let mut participants_guard = self.participants.lock().await;

         if let Some(session) = participants_guard.remove(client_id) {
             info!("Removed session for client {} from tournament {}", client_id, self.tournament_id);

             // TODO: Persist final state? Delete from cache?
             // cache_delete_typing_session(&self.tournament_id, client_id).await;

             // Release lock before await calls
             drop(participants_guard);

             socket.leave(self.tournament_id.clone());


             // Broadcast that user left (send ClientSchema or just ID)
             let user_data = session.client; // Use the client data from the removed session
            if let Err(e) = self.io.to(self.tournament_id.clone()).emit("user:left", &user_data).await {
                 warn!("Failed to broadcast user:left for {}: {}", client_id, e);
             }

             // Broadcast updated tournament state
             self.broadcast_tournament_update().await;

             // Send leave response only if it wasn't a disconnect event
             if !is_disconnect {
                 let response = ApiResponse::<()>::success("Left tournament successfully", None);
                 if let Err(e) = socket.emit("leave:response", &response) {
                    warn!("Failed to send leave:response to {}: {}", client_id, e);
                 }
             }

             Ok(())

         } else {
              warn!("Leave/disconnect request for client {} but no session found in manager.", client_id);
               // Send error response only if it wasn't a disconnect (don't respond to ghosts)
              if !is_disconnect {
                 let response = ApiResponse::<()>::error("You are not in this tournament session.");
                  let _ = socket.emit("leave:response", &response);
              }
             // Release lock
              drop(participants_guard);
              Err(anyhow::anyhow!("Client session not found"))
         }
    }

     /// Fetches current state and broadcasts `tournament:update` to the room.
     async fn broadcast_tournament_update(self: &Arc<Self>) {
         let tournament_state_data;
         let participants_list;

         { // Lock state briefly to get consistent data
             let state_guard = self.tournament_state.lock().await;
             tournament_state_data = state_guard.clone(); // Clone the session data

             let participants_guard = self.participants.lock().await;
             participants_list = participants_guard.values().cloned().collect::<Vec<_>>(); // Clone participant data
         } // Locks released

        let update_payload = TournamentUpdateSchema::new(tournament_state_data, participants_list);

         if let Err(e) = self.io.to(self.tournament_id.clone()).emit("tournament:update", &update_payload).await {
              warn!("Failed to broadcast tournament:update for {}: {}", self.tournament_id, e);
         } else {
              info!("Broadcasted tournament:update for {}", self.tournament_id);
         }
     }

     // TODO: Add cleanup logic if needed (e.g., when tournament ends, remove from registry)
     // pub async fn cleanup(self: &Arc<Self>, registry: TournamentRegistry) {
     //     info!("Cleaning up manager for tournament {}", self.tournament_id);
     //     // Cancel scheduled task if still running?
     //     if let Some(handle) = self.scheduler_handle.lock().unwrap().take() {
     //         handle.abort();
     //     }
     //     // Remove from registry
     //     let mut reg = registry.lock().await;
     //     reg.remove(&self.tournament_id);
     //     // Disconnect remaining sockets?
     //     // let sockets = self.sockets.lock().await;
     //     // for socket in sockets.values() { let _ = socket.disconnect(); }
     // }
}

// Ensure the manager cleans up if dropped (though explicit cleanup might be better)
// impl Drop for TournamentManager {
//     fn drop(&mut self) {
//         info!("Dropping TournamentManager for {}", self.tournament_id);
//         // Abort task if handle exists and isn't None
//         if let Some(handle) = self.scheduler_handle.lock().unwrap().as_ref() {
//             handle.abort();
//         }
//     }
// }