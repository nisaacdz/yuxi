use std::{collections::HashMap, sync::Arc, time::Duration};
use anyhow::Result;
use chrono::{TimeDelta, Utc};
use models::schemas::{tournament::TournamentSession, typing::{TournamentUpdateSchema, TypingSessionSchema}, user::ClientSchema};
use sea_orm::DatabaseConnection;
use socketioxide::{extract::SocketRef, SocketIo};
use tracing::{error, info, warn};

use crate::cache::Cache;

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
                tournament_state.lock().await.started_at = Some(Utc::now());
            };
            
            match crate::scheduler::schedule_new_task(
                tournament_id.to_string(), // Task ID
                task,                      // Task future
                task_scheduled_for,        // Execution time
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
        let tournament_id = self.tournament_id.clone();
        let client_id = client.id.clone();
        info!("Handling connection for client {} to tournament {}", client_id, tournament_id);

        // --- Check Join Deadline ---
        let current_tournament_state = self.tournament_state.lock().await.clone(); // Clone the state data
        let now = Utc::now();
        let join_deadline = current_tournament_state.scheduled_for - TimeDelta::seconds(JOIN_DEADLINE_SECONDS);

        if now >= join_deadline {
            warn!("Client {} attempted to join {} after deadline", client_id, tournament_id);
            let _ = socket.emit("join:response", &ApiResponse::<()>::error("Tournament join deadline has passed."));
            return Err(anyhow::anyhow!("Join deadline passed"));
        }

        // --- Create and Store Typing Session ---
        let new_session = TypingSessionSchema::new(client.clone(), tournament_id.clone());
        { // Lock participants map briefly
            let mut participants_guard = self.participants.lock().await;
            // Optional: Handle case where user might already be in the map (e.g., reconnect)
            if participants_guard.contains_key(&client_id) {
                 warn!("Client {} already has a session in tournament {}. Overwriting.", client_id, tournament_id);
                 // Or decide on different reconnect logic
            }
            participants_guard.insert(client_id.clone(), new_session.clone());
            info!("Added session for client {} to tournament {}", client_id, tournament_id);
        }
        

        socket.join(tournament_id.clone());
        info!("Socket {} joined room {}", socket.id, tournament_id);

        let join_response = ApiResponse::success("Joined tournament successfully", Some(&current_tournament_state));
        socket.emit("join:response", &join_response)?;


        self.broadcast_tournament_update().await;


        self.register_socket_listeners(client.clone(), socket.clone());

        Ok(())
    }


    fn register_socket_listeners(self: &Arc<Self>, client: ClientSchema, socket: SocketRef) {
        let tournament_id = socket.ns().trim_start_matches("/tournament/").to_string();
        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
        info!(
            "Socket.IO connected to dynamic namespace {} : {:?}",
            tournament_id, client
        );

        handlers::handle_join(
            tournament_id.to_owned(),
            self.io.clone(),
            socket.clone(),
            self.conn.clone(),
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


    // --- Internal Helper Methods ---

    /// Internal logic for handling character typing.
    async fn handle_typing_internal(self: &Arc<Self>, client_id: &str, typed_chars: Vec<char>) {
        if typed_chars.is_empty() {
            // warn!("Empty typing event for {}", client_id); // Maybe too noisy
            return;
        }

        let tournament_state = self.tournament_state.lock().await; // Lock state
        let challenge_text_bytes = match &tournament_state.text {
            Some(text) if !text.is_empty() => text.as_bytes(),
            _ => {
                warn!("Typing event for {} but tournament text is not ready/available.", client_id);
                // Optionally emit an error back to the specific client?
                // let _ = self.io.to(socket_id).emit("typing:error", ...); // Need socket id here
                return; // Cannot process typing without text
            }
        };

        if tournament_state.started_at.is_none() {
             warn!("Typing event for {} before tournament start.", client_id);
             // Optionally emit an error back to the specific client?
             return;
        }

        // Drop the state lock before locking participants
        drop(tournament_state);

        let mut participants_guard = self.participants.lock().await; // Lock participants
        if let Some(session) = participants_guard.get_mut(client_id) {
            let now = Utc::now();
             // Use the existing pure logic function
            let updated_session = process_typing_input(session.clone(), typed_chars, challenge_text_bytes, now); // Clone session for update

            // Update the session in the map
            *session = updated_session.clone(); // Update in place

             // TODO: Persist progress update maybe? Or only at the end?
             // cache_set_typing_session(updated_session.clone()).await; // Update cache if needed frequently

             // Prepare broadcast data (just the updated session)
             let response = ApiResponse::success("Progress updated", Some(&updated_session)); // Send updated session

            // Release participants lock *before* await on broadcast
             drop(participants_guard);

             // Broadcast update to the room
            if let Err(e) = self.io.to(self.tournament_id.clone()).emit("typing:update", &response).await {
                 warn!("Failed to broadcast typing:update for {}: {}", client_id, e);
            }

        } else {
             warn!("Typing event for client {} but no session found in manager.", client_id);
             // Drop lock if not found
             drop(participants_guard);
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

             // Leave the socket room (important!)
            if let Err(e) = socket.leave(self.tournament_id.clone()) {
                warn!("Failed to leave room {} for socket {}: {}", self.tournament_id, socket.id, e);
                // Continue cleanup anyway
            } else {
                 info!("Socket {} left room {}", socket.id, self.tournament_id);
            }


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