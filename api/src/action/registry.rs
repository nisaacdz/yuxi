use models::schemas::user::ClientSchema;
use sea_orm::DatabaseConnection;
use socketioxide::SocketIo;
use socketioxide::extract::SocketRef;
use tracing::{error, info, warn};

use crate::action::manager::TournamentManager;
use crate::cache::{TournamentRegistry, TypingSessionRegistry};

pub fn register_tournament_namespace(
    io: SocketIo,
    conn: DatabaseConnection,
    tournament_registry: TournamentRegistry,
    session_registry: TypingSessionRegistry,
) {
    let res = io.clone().dyn_ns(
        "/tournament/{tournament_id}",
        async move |socket: SocketRef| {
            let tournament_id = socket.ns().trim_start_matches("/tournament/").to_owned();

            let registry = tournament_registry.clone();
            let conn = conn.clone();
            let socket = socket.clone();

            let client = match socket.req_parts().extensions.get::<ClientSchema>() {
                Some(client) => client.clone(),
                None => {
                    error!(
                        "ClientSchema not found in socket extensions for ID: {}",
                        socket.id
                    );
                    let _ = socket.disconnect();
                    return;
                }
            };

            info!(
                "Socket.IO connected for tournament '{}': Client: {:?}",
                tournament_id, client.id
            );

            let tournament =
                match app::persistence::tournaments::get_tournament(&conn, tournament_id.clone())
                    .await
                {
                    Ok(Some(tournament)) => tournament,
                    Ok(None) => {
                        error!("Tournament with ID '{}' not found", tournament_id);
                        let _ = socket.disconnect();
                        return;
                    }
                    Err(e) => {
                        error!("Error fetching tournament '{}': {}", tournament_id, e);
                        let _ = socket.disconnect();
                        return;
                    }
                };

            let typing_text =
                match app::persistence::text::get_or_generate_text(&conn, &tournament.id).await {
                    Ok(text) => text,
                    Err(e) => {
                        error!(
                            "Error fetching text for tournament '{}': {}",
                            tournament_id, e
                        );
                        let _ = socket.disconnect();
                        return;
                    }
                };

            let manager = registry.get_or_init(tournament_id.to_owned(), move || {
                TournamentManager::new(
                    tournament,
                    typing_text,
                    conn.clone(),
                    io,
                    session_registry.clone(),
                    tournament_registry.clone(),
                )
            });

            if let Err(e) = manager.connect(socket.clone()).await {
                warn!("Error handling client connection for {}: {}", client.id, e);
                // Error response already sent within handle_client_connection
                let _ = socket.disconnect();
            }
        },
    );

    match res {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to setup dynamic namespace: {e}");
            panic!("Failed to setup dynamic namespace: {e}")
        }
    }
}
