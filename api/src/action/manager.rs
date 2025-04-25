// src/tournament/manager.rs (New file for the TournamentManager)
use std::{collections::HashMap, sync::{Arc, Mutex as StdMutex}, time::Duration}; // Use std::sync::Mutex if locks are short and not held across awaits
// Or use tokio::sync::Mutex if locks might be held across .await points inside the manager's methods
use anyhow::{Context, Result};
use chrono::{TimeDelta, Utc};
use sea_orm::DatabaseConnection;
use socketioxide::{extract::SocketRef, SocketIo};
use tokio::task::JoinHandle; // For holding the scheduler task handle
use tracing::{error, info, warn};

// How long before scheduled start can users join?
const JOIN_DEADLINE_SECONDS: i64 = 15;
// Inactivity timeout duration
const INACTIVITY_TIMEOUT_DURATION: Duration = Duration::from_secs(30);
// Frequency Monitor settings
const DEBOUNCE_DURATION: Duration = Duration::from_millis(100);
const MAX_PROCESS_WAIT: Duration = Duration::from_secs(1);
const MAX_PROCESS_STACK_SIZE: u32 = 15;

pub struct TournamentManager {
    tournament_id: String,
    // Use tokio::sync::Mutex if awaits happen while holding the lock, otherwise std::sync::Mutex is fine.
    // Since typing updates modify this frequently from different sockets, tokio::sync::Mutex might be safer.
    tournament_state: Arc<tokio::sync::Mutex<TournamentSession>>,
    participants: Arc<tokio::sync::Mutex<HashMap<String, TypingSessionSchema>>>, // client_id -> session
    io: SocketIo,
    conn: DatabaseConnection,
    // Optional: Handle for the scheduled task (e.g., to cancel if needed)
    scheduler_handle: Arc<StdMutex<Option<JoinHandle<()>>>>,
    // Keep track of connected sockets specific to this manager/tournament
    // Needed for graceful shutdown or specific socket actions if required beyond room broadcasts
    // sockets: Arc<tokio::sync::Mutex<HashMap<String, SocketRef>>>, // socket.id -> socket // Consider if needed
}

impl TournamentManager {
    /// Initializes the TournamentManager for a given tournament ID.
    /// Fetches initial data, sets up the tournament session in memory,
    /// and schedules the task to start the tournament.
    pub async fn init(
        tournament_id: &str,
        conn: DatabaseConnection,
        io: SocketIo,
    ) -> Result<Self> {
        info!("Initializing TournamentManager for {}", tournament_id);
        // --- 1. Fetch Tournament Base Data ---
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

        // --- 2. Create Initial In-Memory State ---
        // Note: Text is None initially, will be populated by the scheduled task.
        // Started_at is also None initially.
        let initial_session = TournamentSession::new(
            tournament_schema.id.clone(),
            tournament_schema.scheduled_for,
            None, // No text yet
        );
        let tournament_state = Arc::new(tokio::sync::Mutex::new(initial_session));
        let participants = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        // --- 3. Schedule the Tournament Start Task ---
        let scheduler_handle = {
            let task_conn = conn.clone();
            let task_io = io.clone();
            let task_tournament_id = tournament_id.to_string();
            let task_tournament_state = tournament_state.clone(); // Clone Arc for the task
            let task_scheduled_for = tournament_schema.scheduled_for;
            let task_participants = participants.clone(); // Clone Arc for the task

            let task = async move {
                info!("Scheduled task running for tournament {}", task_tournament_id);
                 match app::persistence::text::get_or_generate_text(&task_conn, &task_tournament_id).await {
                    Ok(text) => {
                        let start_time = Utc::now();
                        let text_arc = Arc::new(text); // Put text in Arc for cheaper cloning if needed

                        // --- Update Manager's Tournament State ---
                        { // Lock state briefly
                            let mut state_guard = task_tournament_state.lock().await;
                            state_guard.text = Some(text_arc.clone()); // Store Arc<String>
                            state_guard.started_at = Some(start_time);
                            info!("Tournament {} state updated with text and start time", task_tournament_id);
                        } // Lock released

                        // --- Prepare Broadcast Data ---
                        // It's crucial to get a consistent snapshot AFTER updating the state
                        let final_tournament_state;
                        let final_participants_list;
                        {
                           final_tournament_state = task_tournament_state.lock().await.clone(); // Clone the session data
                           let parts_guard = task_participants.lock().await;
                           final_participants_list = parts_guard.values().cloned().collect::<Vec<_>>(); // Clone participant data
                        } // Locks released

                        let tournament_update = TournamentUpdateSchema::new(
                             final_tournament_state, // Use the cloned state
                             final_participants_list, // Use the cloned list
                        );


                        // --- Broadcast Start/Update ---
                        info!("Broadcasting tournament:start for {}", task_tournament_id);
                         match task_io
                            .to(task_tournament_id.clone())
                            .emit("tournament:start", &tournament_update) // Send the comprehensive update
                         {
                             Ok(_) => info!("Successfully broadcast tournament:start for {}", task_tournament_id),
                             Err(e) => warn!("Failed to broadcast tournament:start for {}: {}", task_tournament_id, e),
                         }
                    }
                    Err(err) => {
                        error!(
                            "Error generating/fetching text for tournament {}: {}",
                            task_tournament_id, err
                        );
                        // How to handle? Broadcast an error? Update state with error?
                        // For now, just log. Participants might see no text.
                         let error_msg = format!("Failed to prepare tournament text: {}", err);
                         match task_io
                             .to(task_tournament_id.clone())
                             .emit("tournament:error", &ApiResponse::<()>::error(&error_msg))
                         {
                             Ok(_) => warn!("Broadcast tournament error message for {}", task_tournament_id),
                             Err(e) => error!("Failed to broadcast tournament error for {}: {}", task_tournament_id, e),
                         }
                    }
                }
            };

            // Schedule the task to run at the designated time
            match scheduler::schedule_new_task(
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
                    // This is likely a critical failure - the tournament won't start properly.
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
            scheduler_handle: Arc::new(StdMutex::new(scheduler_handle)),
            // sockets: Arc::new(tokio::sync::Mutex::new(HashMap::new())), // Initialize if using
        })
    }

    /// Handles a new client connection for this specific tournament.
    /// This replaces the old `handle_join` and the event listener setup part of `enter_tournament`.
    /// Corresponds to the desired `lock_on` functionality.
    pub async fn handle_client_connection(
        self: &Arc<Self>, // Take Arc<Self> to easily clone for event handlers
        client: ClientSchema,
        socket: SocketRef,
    ) -> Result<()> { // Return Result to indicate success/failure of handling the connection

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
        } // Lock released


        // --- Join Socket Room & Emit Success ---
        socket.join(tournament_id.clone())?; // Propagate socket errors
        info!("Socket {} joined room {}", socket.id, tournament_id);

        // Send successful join response *with current tournament state*
        let join_response = ApiResponse::success("Joined tournament successfully", Some(&current_tournament_state));
        socket.emit("join:response", &join_response)?;


        // --- Broadcast User Joined and Tournament Update ---
        self.broadcast_tournament_update().await; // Use a helper to broadcast the current state


        // --- Set up Event Listeners for this Socket ---
        self.register_socket_listeners(client.clone(), socket.clone());


        // --- Add socket to internal tracking if needed ---
        // {
        //     let mut sockets_guard = self.sockets.lock().await;
        //     sockets_guard.insert(socket.id.to_string(), socket.clone());
        // }


        Ok(())
    }


    /// Registers the 'type-character', 'leave-tournament', and 'disconnect' handlers for a specific socket.
    fn register_socket_listeners(self: &Arc<Self>, client: ClientSchema, socket: SocketRef) {
        let client_id = client.id.clone();
        let tournament_id = self.tournament_id.clone();

        // --- Timeout Monitor Setup ---
        let timeout_monitor = {
            let manager_clone = self.clone(); // Clone Arc<Self> for the timeout handler
            let socket_clone = socket.clone(); // Clone SocketRef for the timeout handler
            let client_clone = client.clone(); // Clone ClientSchema for the timeout handler

            Arc::new(TimeoutMonitor::new(
                // on_timeout: Function to execute when timeout occurs
                async move {
                     info!("Inactivity timeout for client {}", client_clone.id);
                     // Call the manager's internal leave/disconnect logic
                     let _ = manager_clone.handle_leave_internal(&client_clone.id, &socket_clone, true).await; // Indicate it's a timeout
                     // Optionally force disconnect even if leave fails
                     let _ = socket_clone.disconnect();
                },
                // after_timeout: Function to execute after the timeout callback finishes (optional)
                async move || { info!("Finished handling timeout for {}", client_clone.id) },
                INACTIVITY_TIMEOUT_DURATION, // Use the defined constant
            ))
        };

        // --- Frequency Monitor Setup ---
        let frequency_monitor = Arc::new(FrequencyMonitor::new(
            DEBOUNCE_DURATION,
            MAX_PROCESS_WAIT,
            MAX_PROCESS_STACK_SIZE as usize, // Cast might be needed depending on FrequencyMonitor definition
        ));

        // --- 1. Typing Handler ---
        socket.on("type-character", {
            let manager_clone = self.clone(); // Clone Arc<Self> for the handler
            let timeout_monitor_clone = timeout_monitor.clone();
            let frequency_monitor_clone = frequency_monitor.clone();
            let client_id_clone = client_id.clone();

             move |socket: SocketRef, Data::<TypeArgs>(TypeArgs { character })| {
                 // Clone Arcs needed inside the innermost async block
                 let manager_clone_inner = manager_clone.clone();
                 let client_id_inner = client_id_clone.clone();
                 let freq_monitor_inner = frequency_monitor_clone.clone();
                 let timeout_monitor_inner = timeout_monitor_clone.clone(); // Clone for the processor task

                 // Spawn a task to handle the processing off the immediate network handler thread
                 tokio::spawn(async move {
                     // This processor runs the frequency monitor and the actual typing logic
                     let processor = async move {
                         let result = freq_monitor_inner.call(character, move |chars: Vec<char>| {
                            // Pass the chars to the manager's internal typing handler
                            // Clone necessary resources again for the handler call
                            let manager_handler_clone = manager_clone_inner.clone();
                            let client_id_handler = client_id_inner.clone();
                             async move {
                                manager_handler_clone.handle_typing_internal(&client_id_handler, chars).await;
                            }
                        }).await;

                         if let Err(e) = result {
                              warn!("Frequency monitor call failed for {}: {}", client_id_inner, e);
                         }
                     };

                     // Wrap the processing with the timeout monitor
                     timeout_monitor_inner.call(processor).await;
                 });
            }
        });
        info!("Registered 'type-character' handler for {}", client_id);

        // --- 2. Leave Handler ---
        socket.on("leave-tournament", {
            let manager_clone = self.clone(); // Clone Arc<Self> for the handler
            let client_id_clone = client_id.clone();
             move |socket: SocketRef| {
                info!("Received 'leave-tournament' from {}", client_id_clone);
                 let manager_leave_clone = manager_clone.clone();
                 let client_id_leave = client_id_clone.clone();
                 // Spawn task to avoid blocking the event loop
                 tokio::spawn(async move {
                      let _ = manager_leave_clone.handle_leave_internal(&client_id_leave, &socket, false).await;
                      // Consider disconnecting after successful leave, like original code?
                      // if result.is_ok() { let _ = socket.disconnect(); }
                 });
             }
        });
         info!("Registered 'leave-tournament' handler for {}", client_id);

        // --- 3. Disconnect Handler ---
        // This handles graceful disconnects initiated by client or server (after leave),
        // and unexpected disconnects (network issues, browser close).
        socket.on_disconnect({
             let manager_clone = self.clone(); // Clone Arc<Self> for the handler
             let client_id_clone = client_id.clone();
             move |socket: SocketRef, reason| {
                info!("Socket disconnected for client {} (reason: {})", client_id_clone, reason);
                 let manager_disconnect_clone = manager_clone.clone();
                 let client_id_disconnect = client_id_clone.clone();
                tokio::spawn(async move {
                     // Use the same leave logic for cleanup, indicating it's a disconnect
                     let _ = manager_disconnect_clone.handle_leave_internal(&client_id_disconnect, &socket, true).await; // `is_disconnect = true`
                     // Remove socket from internal tracking if used
                     // let _ = manager_disconnect_clone.remove_socket(&socket.id).await;
                 });
             }
        });
         info!("Registered 'disconnect' handler for {}", client_id);
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