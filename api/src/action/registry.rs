// src/tournament/mod.rs (or wherever you define the main entry)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sea_orm::DatabaseConnection;
use socketioxide::extract::SocketRef;
use socketioxide::SocketIo;
use tracing::{error, info, warn};

// Type alias for the central registry holding active tournament managers
pub type TournamentRegistry = Arc<Mutex<HashMap<String, Arc<TournamentManager>>>>;

// Function to initialize and register the dynamic namespace handler
pub fn register_tournament_namespace(
    io: &SocketIo,
    conn: DatabaseConnection,
    registry: TournamentRegistry,
) {
    io.ns("/tournament/*", move |socket: SocketRef, tournament_id: String| {
        // Clone resources needed for the connection handler
        let registry = registry.clone();
        let conn = conn.clone();
        let io_clone = io.clone(); // Clone io for use inside the async block

        // Spawn a task to handle this specific connection
        tokio::spawn(async move {
            // --- Get Client Info ---
            let client = match socket.req_parts().extensions.get::<ClientSchema>() {
                Some(client) => client.clone(),
                None => {
                    error!("ClientSchema not found in socket extensions for ID: {}", socket.id);
                    // Optionally emit an error back to the client before closing
                    let _ = socket.disconnect(); // Disconnect if essential info is missing
                    return;
                }
            };
            info!("Socket.IO connected for tournament '{}': Client: {:?}", tournament_id, client.id);


            // --- Get or Initialize Tournament Manager ---
            let manager = match get_or_init_manager(registry, tournament_id.clone(), conn, io_clone).await {
                Ok(manager) => manager,
                Err(e) => {
                    error!("Failed to get or initialize manager for tournament {}: {}", tournament_id, e);
                    // Emit an error to the client
                    let _ = socket.emit("join:response", &super::state::ApiResponse::<()>::error("Failed to initialize tournament session."));
                    let _ = socket.disconnect();
                    return;
                }
            };

            // --- Delegate Connection Handling to the Manager ---
            // This manager instance now handles join logic, event listeners, etc. for this socket
            if let Err(e) = manager.handle_client_connection(client, socket.clone()).await {
                 warn!("Error handling client connection for {}: {}", client.id, e);
                 // Error response already sent within handle_client_connection
                 let _ = socket.disconnect();
            }

            // --- Handle Disconnect ---
            // The primary disconnect logic is now inside handle_client_connection's 'disconnect' listener.
            // This outer listener catches disconnects that might happen *before* or *during* the initial setup.
             socket.on_disconnect(move |socket: SocketRef, reason| {
                 info!("Outer disconnect handler triggered for socket {}: {}", socket.id, reason);
                 // We might need to ensure cleanup happens even if connection failed early.
                 // The manager's internal disconnect handler should ideally handle clean removal.
                 // Consider if additional cleanup is needed here based on where the disconnect occurred.
             });
        });
    });
}

// Helper function to retrieve an existing manager or initialize a new one
async fn get_or_init_manager(
    registry: TournamentRegistry,
    tournament_id: String,
    conn: DatabaseConnection,
    io: SocketIo,
) -> Result<Arc<TournamentManager>, anyhow::Error> {
    // --- Lock the registry to safely check/insert ---
    let mut registry_guard = registry.lock().await; // Use tokio::sync::Mutex if registry is shared across awaits

    // --- Check if manager already exists ---
    if let Some(manager) = registry_guard.get(&tournament_id) {
        return Ok(manager.clone()); // Clone the Arc, not the manager itself
    }

    // --- Manager doesn't exist, initialize a new one ---
    let manager = TournamentManager::init(&tournament_id, conn, io).await?; // Propagate errors from init

    let manager_arc = Arc::new(manager);
    registry_guard.insert(tournament_id.clone(), manager_arc.clone());

    Ok(manager_arc)

    // MutexGuard is dropped here, releasing the lock
}

// --- Add other necessary modules/imports ---
// E.g., pub mod manager; pub mod state; pub mod logic; etc.