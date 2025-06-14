use std::collections::HashMap;

use app::{core::TournamentManager, state::AppState};
use models::schemas::user::ClientSchema;
use socketioxide::extract::SocketRef;
use tracing::{error, info, warn};

pub fn register_tournament_namespace(app_state: AppState) {
    let _ = app_state
        .socket_io
        .clone()
        .ns("/", async move |socket: SocketRef| {
            let query_string = socket.req_parts().uri.query().unwrap_or_default();
            let params_map =
                url::form_urlencoded::parse(query_string.as_bytes()).collect::<HashMap<_, _>>();

            let tournament_id = match params_map.get("id") {
                Some(id) => id.clone(),
                None => {
                    error!(
                        "No tournament_id provided in handshake query for socket {}",
                        socket.id
                    );
                    let _ = socket.disconnect();
                    return;
                }
            };

            let spectator: bool = params_map
                .get("spectator")
                .and_then(|val_str| val_str.parse::<bool>().ok())
                .unwrap_or(false);

            let app_state = app_state.clone();
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

            let tournament = match app::persistence::tournaments::get_tournament(
                &app_state.conn,
                tournament_id.to_string(),
            )
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

            let tournament_registry = app_state.tournament_registry.clone();

            let manager = tournament_registry.get_or_init(tournament_id.to_string(), move || {
                TournamentManager::new(tournament, app_state)
            });

            if let Err(e) = manager.connect(socket.clone(), spectator).await {
                warn!("Error handling client connection for {}: {}", client.id, e);
                // Error response already sent within handle_client_connection
                let _ = socket.disconnect();
            }
        });
}
