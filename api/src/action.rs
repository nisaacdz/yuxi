use std::{collections::HashMap, sync::Arc};

use app::{
    core::{TournamentManager, WsFailurePayload},
    state::AppState,
};
use models::schemas::user::{AuthSchema, TournamentRoomMember};
use socketioxide::extract::{HttpExtension, SocketRef};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{decode_noauth, encode_noauth};

pub fn register_tournament_namespace(app_state: AppState) {
    let _ = app_state.socket_io.clone().ns(
        "/",
        async move |HttpExtension(auth_state): HttpExtension<AuthSchema>, socket: SocketRef| {
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

            let anonymous: bool = params_map
                .get("anonymous")
                .and_then(|val_str| val_str.parse::<bool>().ok())
                .unwrap_or(false);

            let mut noauth = String::from("not-set");

            let tournament_room_member = match &auth_state.user {
                Some(user) => TournamentRoomMember::from_user(user, anonymous, !spectator),
                None => match socket
                    .req_parts()
                    .headers
                    .get("x-noauth-unique")
                    .map(|value| decode_noauth(value.as_ref()))
                    .flatten()
                {
                    Some(id) => TournamentRoomMember {
                        id,
                        user: None,
                        participant: !spectator,
                    },
                    None => {
                        let id = Uuid::new_v4().to_string();
                        noauth = encode_noauth(&id);
                        TournamentRoomMember {
                            id,
                            user: None,
                            participant: !spectator,
                        }
                    }
                },
            };

            socket.extensions.insert(Arc::new(tournament_room_member));

            let app_state = app_state.clone();
            let socket = socket.clone();

            let member = match socket.extensions.get::<Arc<TournamentRoomMember>>() {
                Some(member) => member.clone(),
                None => {
                    error!(
                        "TournamentRoomMember not found in socket extensions for ID: {}",
                        socket.id
                    );
                    let _ = socket.disconnect();
                    return;
                }
            };

            info!(
                "Socket.IO connected for tournament '{}': Member: {:?}",
                tournament_id, member.id
            );

            let tournament_registry = app_state.tournament_registry.clone();

            let manager = match tournament_registry.get(&tournament_id) {
                Some(manager) => manager,
                None => {
                    let tournament = match app::persistence::tournaments::get_tournament(
                        &app_state.conn,
                        tournament_id.to_string(),
                    )
                    .await
                    {
                        Ok(Some(tournament)) if tournament.ended_at.is_some() => {
                            error!("Tournament with ID '{}' has already ended", tournament_id);
                            socket
                                .emit(
                                    "join:failure",
                                    &WsFailurePayload::new(1005, "Tournament has already ended"),
                                )
                                .unwrap();
                            let _ = socket.disconnect();
                            return;
                        }
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

                    info!(
                        "Creating new TournamentManager for tournament '{}'",
                        tournament_id
                    );

                    tournament_registry.get_or_init(tournament_id.to_string(), || {
                        TournamentManager::new(tournament, app_state.clone())
                    })
                }
            };

            if let Err(e) = manager.connect(socket.clone(), spectator, noauth).await {
                warn!("Error handling member connection for {}: {}", member.id, e);
                let _ = socket.disconnect();
            }
        },
    );
}
