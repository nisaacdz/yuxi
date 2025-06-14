use anyhow::Result;
use chrono::{TimeDelta, Utc};
use models::schemas::{
    tournament::{TournamentSchema, TournamentSession},
    typing::{TournamentUpdateSchema, TypingSessionSchema},
    user::ClientSchema,
};
use sea_orm::DatabaseConnection;
use socketioxide::{
    SocketIo,
    extract::{Data, SocketRef},
};
use std::{sync::Arc, time::Duration};
use tracing::{error, info, warn};

use crate::{
    action::{
        TypeArgs,
        handlers::{handle_timeout, handle_typing},
        moderation::FrequencyMonitor,
    },
    cache::{TournamentRegistry, TypingSessionRegistry},
};

use app::cache::Cache;

use crate::{ApiResponse, action::timeout::TimeoutMonitor};

const JOIN_DEADLINE: Duration = Duration::from_secs(15);
const INACTIVITY_TIMEOUT_DURATION: Duration = Duration::from_secs(30);
const DEBOUNCE_DURATION: Duration = Duration::from_millis(100);
const MAX_PROCESS_WAIT: Duration = Duration::from_secs(5);
const MAX_PROCESS_STACK_SIZE: usize = 15;

#[derive(Clone)]
pub struct TournamentManager {
    tournament_id: Arc<String>,
    tournament: TournamentSchema,
    tournament_state: Arc<tokio::sync::Mutex<TournamentSession>>,
    participants: Cache<TypingSessionSchema>,
    io: SocketIo,
    conn: DatabaseConnection,
    session_registry: TypingSessionRegistry,
    typing_text: Arc<String>,
    tournament_registry: TournamentRegistry,
}

impl TournamentManager {
    pub fn new(
        tournament: TournamentSchema,
        typing_text: String,
        conn: DatabaseConnection,
        io: SocketIo,
        session_registry: TypingSessionRegistry,
        tournament_registry: TournamentRegistry,
    ) -> Self {
        info!("Initializing TournamentManager for {}", &tournament.id);

        let initial_session = TournamentSession::new(
            tournament.id.clone(),
            tournament.scheduled_for,
            Some(typing_text.clone()),
        );

        let tournament_state = Arc::new(tokio::sync::Mutex::new(initial_session));

        let typing_text = Arc::new(typing_text);

        let participants = Cache::new();

        {
            let task_tournament_id = tournament.id.to_string();
            let tournament_state = tournament_state.clone();
            let task_scheduled_for = tournament.scheduled_for;
            let participants = participants.clone();
            let tournament_registry = tournament_registry.clone();
            let io = io.clone();

            let task = async move {
                info!(
                    "Scheduled task running for tournament {}",
                    &task_tournament_id
                );
                if participants.count() > 0 {
                    let mut tlck = tournament_state.lock().await;
                    tlck.started_at = Some(Utc::now());
                    // emit tournament start event to the room
                    info!(
                        "Starting tournament {} with {} participants",
                        &task_tournament_id,
                        participants.count()
                    );
                    io.to(task_tournament_id)
                        .emit("tournament:start", &*tlck)
                        .await
                        .expect("Failed to emit tournament start event");
                } else {
                    Self::cleanup(tournament_registry, &task_tournament_id);
                }
            };

            let tournament_id = tournament.id.clone();
            tokio::task::spawn(async move {
                match crate::scheduler::schedule_new_task(
                    tournament_id.clone(),
                    task,
                    task_scheduled_for,
                )
                .await
                {
                    Ok(handle) => {
                        info!(
                            "Successfully scheduled start task for tournament {}",
                            &tournament_id
                        );
                        Ok(handle)
                    }
                    Err(schedule_err) => {
                        error!(
                            "Failed to schedule task for tournament {}: {}",
                            &tournament_id, schedule_err
                        );
                        Err(anyhow::anyhow!(
                            "Failed to schedule tournament start task: {}",
                            schedule_err
                        ))
                    }
                }
            })
        };

        Self {
            tournament_id: Arc::new(tournament.id.to_string()),
            tournament,
            tournament_state,
            participants,
            io,
            conn,
            typing_text,
            session_registry,
            tournament_registry,
        }
    }

    pub async fn connect(self: &Arc<Self>, socket: SocketRef) -> Result<()> {
        let now = Utc::now();

        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();

        {
            let tournament_state = self.tournament_state.lock().await;

            if !self.participants.contains_key(&client.id)
                && (tournament_state.started_at.is_some()
                    || tournament_state.scheduled_for - now
                        < TimeDelta::from_std(JOIN_DEADLINE).unwrap())
            {
                error!(client_id = %client.id, "Tournament {} has already started or is not scheduled.", self.tournament_id);
                let k = socket.emit(
                    "join:response",
                    &ApiResponse::<()>::error(
                        "Tournament has already started or is not scheduled.",
                    ),
                );

                if let Err(e) = k {
                    warn!("Failed to send join response to {}: {}", client.id, e);
                }

                return Err(anyhow::anyhow!(
                    "Tournament has already started or is not scheduled."
                ));
            }
        }

        let tournament_id = &self.tournament_id;

        info!(
            "Handling connection for client {} to tournament {}",
            &client.id, tournament_id
        );

        self.participants.get_or_insert(&client.id, || {
            TypingSessionSchema::new(client.clone(), tournament_id.to_string())
        });

        // Great!, this will set the global session registry so that the newest session is added
        self.session_registry.set_session(
            &client.id,
            TypingSessionSchema::new(client.clone(), tournament_id.to_string()),
        );

        socket.join(tournament_id.to_string());

        socket.emit("join:response", &ApiResponse::success("Joined successfully", Some(&self.tournament))).ok();

        self.broadcast_tournament_update().await;

        self.register_socket_listeners(socket.clone());

        info!(
            "Client {} connected to tournament {}",
            &client.id, tournament_id
        );

        Ok(())
    }

    fn register_socket_listeners(self: &Arc<Self>, socket: SocketRef) {
        let tournament_id = &self.tournament_id;
        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
        info!(
            "Socket.IO connected to tournament {} : {:?}",
            tournament_id, client.id
        );

        {
            // wait period before processing a new character
            let debounce_duration = DEBOUNCE_DURATION;
            // user should only experience at worst 3s lag time
            // but will likely be in millis under normal circumstances
            let max_process_wait = MAX_PROCESS_WAIT;
            // processing shouldn't lag behind by more than 15 chars from current position
            // but will likely be instantaneous under normal circumstances
            let max_process_stack_size = MAX_PROCESS_STACK_SIZE;
            let cleanup_wait_duration = INACTIVITY_TIMEOUT_DURATION;
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

            let cache = self.participants.clone();

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
            let manager = self.clone();

            socket.on("leave-tournament", async move |socket: SocketRef| {
                let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
                info!(
                    "Received leave request from client {} in tournament {}",
                    client.id, manager.tournament_id
                );

                // Handle leave request
                if let Err(e) = manager.handle_leave_internal(&client.id, &socket).await {
                    warn!("Failed to handle leave request for {}: {}", client.id, e);
                    let response = ApiResponse::<()>::error("Failed to leave tournament.");
                    let _ = socket.emit("leave:response", &response);
                }
            });
        }

        {
            socket.on("disconnect", async move |socket: SocketRef| {
                info!("Socket.IO disconnected: {:?}", socket.id);
                // Will likely reconnect, so we handle it gracefully
            });
        }
    }

    async fn handle_leave_internal(
        self: &Arc<Self>,
        client_id: &str,
        socket: &SocketRef,
    ) -> Result<()> {
        info!("Handling leave/disconnect for client {}", client_id);

        if let Some(session) = self.participants.delete_data(client_id) {
            info!(
                "Removed session for client {} from tournament {}",
                client_id, self.tournament_id
            );

            socket.leave(self.tournament_id.to_string());

            let user_data = session.client;
            if let Err(e) = self
                .io
                .to(self.tournament_id.to_string())
                .emit("user:left", &user_data)
                .await
            {
                warn!("Failed to broadcast user:left for {}: {}", client_id, e);
            }

            self.broadcast_tournament_update().await;

            let response = ApiResponse::<()>::success("Left tournament successfully", None);
            if let Err(e) = socket.emit("leave:response", &response) {
                warn!("Failed to send leave:response to {}: {}", client_id, e);
            }

            Ok(())
        } else {
            warn!(
                "Leave/disconnect request for client {} but no session found in manager.",
                client_id
            );
            let response = ApiResponse::<()>::error("You are not in this tournament session.");
            let _ = socket.emit("leave:response", &response);
            Err(anyhow::anyhow!("Client session not found"))
        }
    }

    /// Fetches current state and broadcasts `tournament:update` to the room.
    async fn broadcast_tournament_update(self: &Arc<Self>) {
        let tournament_state_data = self.tournament_state.lock().await.clone();

        let participants_list = self.participants.values();

        let update_payload = TournamentUpdateSchema::new(tournament_state_data, participants_list);

        if let Err(e) = self
            .io
            .to(self.tournament_id.to_string())
            .emit("tournament:update", &update_payload)
            .await
        {
            warn!(
                "Failed to broadcast tournament:update for {}: {}",
                self.tournament_id, e
            );
        } else {
            info!("Broadcasted tournament:update for {}", self.tournament_id);
        }
    }

    pub fn cleanup(tournament_registry: TournamentRegistry, tournament_id: &str) {
        info!("Cleaning up manager for tournament {}", tournament_id);
        tournament_registry.evict(tournament_id);
    }
}
