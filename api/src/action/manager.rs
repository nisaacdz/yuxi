use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use models::schemas::{
    tournament::{TournamentSchema, TournamentSession},
    typing::TypingSessionSchema,
    user::ClientSchema,
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use socketioxide::{
    SocketIo,
    extract::{Data, SocketRef},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, Notify};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    action::moderation::FrequencyMonitor,
    cache::{TournamentRegistry, TypingSessionRegistry},
};

use app::cache::Cache;

use crate::action::timeout::TimeoutMonitor;

const JOIN_DEADLINE: Duration = Duration::from_secs(15);
const INACTIVITY_TIMEOUT_DURATION: Duration = Duration::from_secs(30);
const DEBOUNCE_DURATION: Duration = Duration::from_millis(100);
const MAX_PROCESS_WAIT: Duration = Duration::from_secs(5);
const MAX_PROCESS_STACK_SIZE: usize = 15;

const UPDATE_ALL_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

#[derive(Serialize, Debug, Clone)]
struct WsFailurePayload {
    code: i32,
    message: String,
}

impl WsFailurePayload {
    fn new(code: i32, message: &str) -> Self {
        Self {
            code: code,
            message: message.to_string(),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ParticipantData {
    client: ClientSchema,
    current_position: usize,
    correct_position: usize,
    total_keystrokes: i32,
    current_speed: f32,
    current_accuracy: f32,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PartialParticipantData {
    #[serde(skip_serializing_if = "Option::is_none")]
    current_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    correct_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_keystrokes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_accuracy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PartialParticipantDataForUpdate {
    client_id: String,
    updates: PartialParticipantData,
}

#[derive(Serialize, Debug, Clone)]
struct UpdateMePayload {
    updates: PartialParticipantData,
}

#[derive(Serialize, Debug, Clone)]
struct UpdateAllPayload {
    updates: Vec<PartialParticipantDataForUpdate>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TournamentData {
    id: String,
    title: String,
    created_at: DateTime<Utc>,
    created_by: String,
    scheduled_for: DateTime<Utc>,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct JoinSuccessPayload {
    data: TournamentData,
    client_id: String,
    participants: Vec<ParticipantData>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MemberJoinedPayload {
    participant: ParticipantData,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MemberLeftPayload {
    client_id: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LeaveSuccessPayload {
    message: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateDataPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_for: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<Option<String>>,
}

#[derive(serde::Deserialize, Debug)]
struct TypeEventPayload {
    character: char,
}

struct TournamentManagerInner {
    tournament_id: Arc<String>,
    tournament_meta: Arc<TournamentSchema>,
    tournament_session_state: Mutex<TournamentSession>,
    participants: Cache<TypingSessionSchema>,
    io: SocketIo,
    db_pool: DatabaseConnection,
    session_registry: TypingSessionRegistry,
    typing_text: Arc<String>,
    tournament_registry: TournamentRegistry,
    update_all_notifier: Arc<Notify>,
}

#[derive(Clone)]
pub struct TournamentManager {
    inner: Arc<TournamentManagerInner>,
    update_all_notifier_clone: Arc<Notify>,
}

impl TournamentManager {
    pub fn new(
        tournament_schema: TournamentSchema,
        typing_text_content: String,
        db_pool: DatabaseConnection,
        io: SocketIo,
        session_registry: TypingSessionRegistry,
        tournament_registry: TournamentRegistry,
    ) -> Self {
        info!(
            "Initializing TournamentManager for {}",
            &tournament_schema.id
        );

        let initial_session_state = Mutex::new(TournamentSession::new(
            tournament_schema.id.clone(),
            tournament_schema.scheduled_for,
            None,
        ));

        let tournament_id_arc = Arc::new(tournament_schema.id.to_string());
        let typing_text_arc = Arc::new(typing_text_content);
        let update_all_notifier = Arc::new(Notify::new());

        let inner_manager_state = TournamentManagerInner {
            tournament_id: tournament_id_arc.clone(),
            tournament_meta: Arc::new(tournament_schema.clone()),
            tournament_session_state: initial_session_state,
            participants: Cache::new(),
            io: io.clone(),
            db_pool,
            session_registry,
            typing_text: typing_text_arc,
            tournament_registry: tournament_registry.clone(),
            update_all_notifier: update_all_notifier.clone(),
        };

        let manager = Self {
            inner: Arc::new(inner_manager_state),
            update_all_notifier_clone: update_all_notifier,
        };

        let debouncer_manager_clone = manager.clone();
        tokio::spawn(async move {
            debouncer_manager_clone
                .run_update_all_debouncer_task()
                .await;
        });

        let start_task_manager_clone = manager.clone();
        let task_tournament_id_for_scheduler = tournament_schema.id.clone();
        let task_scheduled_for_time = tournament_schema.scheduled_for;

        tokio::task::spawn(async move {
            match crate::scheduler::schedule_new_task(
                task_tournament_id_for_scheduler.clone(),
                async move {
                    start_task_manager_clone
                        .execute_tournament_start_logic()
                        .await;
                },
                task_scheduled_for_time,
            )
            .await
            {
                Ok(_handle) => {
                    info!(
                        "Successfully scheduled start task for tournament {} at {}",
                        task_tournament_id_for_scheduler, task_scheduled_for_time
                    );
                }
                Err(schedule_err) => {
                    error!(
                        "Failed to schedule start task for tournament {}: {}",
                        task_tournament_id_for_scheduler, schedule_err
                    );
                }
            }
        });

        manager
    }

    // Task for debouncing "update:all" events
    async fn run_update_all_debouncer_task(self: Self) {
        loop {
            // Wait for a notification OR a timeout.
            tokio::select! {
                _ = self.update_all_notifier_clone.notified() => {
                    // Notification received, debounce a bit.
                    sleep(UPDATE_ALL_DEBOUNCE_DURATION).await;
                }
                // Fallback periodic update, e.g., every 5x debounce duration
                _ = sleep(UPDATE_ALL_DEBOUNCE_DURATION * 5) => {
                    // info!("Debouncer periodic check for tournament {}", self.inner.tournament_id);
                }
            }

            let inner_snapshot = self.inner.clone();

            let all_sessions: Vec<TypingSessionSchema> = inner_snapshot.participants.values();

            if all_sessions.is_empty() {
                continue;
            }

            let updates_for_all: Vec<PartialParticipantDataForUpdate> = all_sessions
                .iter()
                .map(|session_data| {
                    let partial_data = PartialParticipantData {
                        current_position: Some(session_data.current_position),
                        correct_position: Some(session_data.correct_position),
                        total_keystrokes: Some(session_data.total_keystrokes),
                        current_speed: Some(session_data.current_speed),
                        current_accuracy: Some(session_data.current_accuracy),
                        started_at: session_data.started_at,
                        ended_at: session_data.ended_at,
                    };
                    PartialParticipantDataForUpdate {
                        client_id: session_data.client.id.clone(),
                        updates: partial_data,
                    }
                })
                .collect();

            if updates_for_all.is_empty() {
                continue;
            }

            let update_all_payload = UpdateAllPayload {
                updates: updates_for_all,
            };
            let tournament_room_id = inner_snapshot.tournament_id.to_string();
            let io_clone = inner_snapshot.io.clone();

            if let Err(e) = io_clone
                .to(tournament_room_id.clone())
                .emit("update:all", &update_all_payload)
                .await
            {
                error!(
                    "Failed to emit update:all for tournament {}: {}",
                    tournament_room_id, e
                );
            }
        }
    }

    async fn execute_tournament_start_logic(self: Self) {
        info!(
            "Scheduled start task executing for tournament {}",
            &*self.inner.tournament_id
        );

        let participant_count = self.inner.participants.count();

        if participant_count > 0 {
            let update_data_payload;
            {
                let mut session_state_guard = self.inner.tournament_session_state.lock().await;
                session_state_guard.started_at = Some(Utc::now());

                update_data_payload = UpdateDataPayload {
                    title: None,
                    scheduled_for: None,
                    description: None,
                    started_at: session_state_guard.started_at,
                    ended_at: session_state_guard.ended_at,
                    text: Some(Some(self.inner.typing_text.to_string())),
                };
            }

            let tournament_id_str = self.inner.tournament_id.to_string();
            let io_clone = self.inner.io.clone();

            info!(
                "Starting tournament {} with {} participants. Emitting update:data.",
                tournament_id_str, participant_count
            );
            if let Err(e) = io_clone
                .to(tournament_id_str.clone())
                .emit("update:data", &update_data_payload)
                .await
            {
                error!("Failed to emit update:data for tournament start: {}", e);
            }
        } else {
            // No participants, cleanup the manager
            // No need to lock session_state if we are just cleaning up due to no participants
            info!(
                "No participants in tournament {}. Cleaning up.",
                &*self.inner.tournament_id
            );
            let registry = self.inner.tournament_registry.clone();
            let id_to_clean = self.inner.tournament_id.clone();
            Self::cleanup(registry, &id_to_clean);
        }
    }

    // Helper to map TypingSessionSchema to the API's ParticipantData
    fn map_session_to_api_participant_data(session: &TypingSessionSchema) -> ParticipantData {
        ParticipantData {
            client: session.client.clone(),
            current_position: session.current_position,
            correct_position: session.correct_position,
            total_keystrokes: session.total_keystrokes,
            current_speed: session.current_speed,
            current_accuracy: session.current_accuracy,
            started_at: session.started_at,
            ended_at: session.ended_at,
        }
    }

    pub async fn connect(self: Self, socket: SocketRef, spectator: bool) -> Result<()> {
        let client_schema = socket
            .req_parts()
            .extensions
            .get::<ClientSchema>()
            .ok_or_else(|| anyhow::anyhow!("ClientSchema not found in socket extensions"))?
            .clone();

        let now = Utc::now();

        // Check join conditions only if the client is not already a participant
        if !self.inner.participants.contains_key(&client_schema.id) {
            let started_at = {
                // Scope for the lock guard
                let session_state_guard = self.inner.tournament_session_state.lock().await;
                session_state_guard.started_at
                // Guard dropped here
            };

            let scheduled_for = self.inner.tournament_meta.scheduled_for;

            let mut can_join = true;
            let mut reason = "Uknown reason";
            if spectator {
                // Spectators can join at any time
                can_join = true;
            } else if started_at.is_some()
                || (scheduled_for - now < TimeDelta::from_std(JOIN_DEADLINE).unwrap())
            {
                // If the tournament has already started
                can_join = false;
                reason = "Tournament no longer accepting participants.";
            }

            if !can_join {
                error!(client_id = %client_schema.id, "Cannot join tournament {}: {}", self.inner.tournament_id, reason);
                let failure_payload = WsFailurePayload::new(1004, reason);
                // Emit failure and return early
                if socket.emit("join:failure", &failure_payload).is_err() {
                    warn!("Failed to send join:failure to client {}", client_schema.id);
                }
                return Err(anyhow::anyhow!(reason));
            }
        }

        info!(
            "Handling connection for client {} to tournament {}",
            &client_schema.id, self.inner.tournament_id
        );

        socket.join(self.inner.tournament_id.to_string());

        let current_tournament_data;
        {
            let t_session_state_guard = self.inner.tournament_session_state.lock().await;
            let t_meta = &self.inner.tournament_meta;
            current_tournament_data = TournamentData {
                id: t_meta.id.clone(),
                title: t_meta.title.clone(),
                created_at: t_meta.created_at,
                created_by: t_meta.created_by.clone(),
                scheduled_for: t_meta.scheduled_for,
                description: t_meta.description.clone(),
                started_at: t_session_state_guard.started_at,
                ended_at: t_session_state_guard.ended_at,
                text: if t_session_state_guard.started_at.is_some() {
                    Some(self.inner.typing_text.to_string())
                } else {
                    None
                },
            };
        }

        let all_participants_api_data: Vec<ParticipantData> = self
            .inner
            .participants
            .values()
            .iter()
            .map(|s| Self::map_session_to_api_participant_data(s))
            .collect();

        let join_success_payload = JoinSuccessPayload {
            data: current_tournament_data,
            client_id: client_schema.id.clone(),
            participants: all_participants_api_data,
        };
        // Emit join:success to the current socket
        if socket.emit("join:success", &join_success_payload).is_err() {
            warn!("Failed to send join:success to client {}", client_schema.id);
        }

        if !spectator {
            // Add or get participant session
            let participant_session =
                self.inner
                    .participants
                    .get_or_insert(&client_schema.id, || {
                        TypingSessionSchema::new(
                            client_schema.clone(),
                            self.inner.tournament_id.to_string(),
                        )
                    });
            // Update the global session registry
            self.inner
                .session_registry
                .set_session(&client_schema.id, participant_session.clone());

            // Broadcast "member:joined" to other clients in the room
            let new_participant_api_data =
                Self::map_session_to_api_participant_data(&participant_session);
            let member_joined_payload = MemberJoinedPayload {
                participant: new_participant_api_data,
            };

            let io_clone = self.inner.io.clone(); // Arc<Inner>, direct access
            let tournament_id_str = self.inner.tournament_id.to_string(); // Arc<String>, direct access

            // Emit to room, excluding the current socket
            if let Err(e) = io_clone
                .to(tournament_id_str)
                .except(socket.id)
                .emit("member:joined", &member_joined_payload)
                .await
            {
                warn!("Failed to broadcast member:joined: {}", e);
            }
        }

        // Register other event listeners for this socket
        self.clone()
            .register_socket_listeners(socket.clone(), spectator);

        info!(
            "Client {} connected to tournament {}",
            &client_schema.id, self.inner.tournament_id
        );

        Ok(())
    }

    /// Processes a sequence of typed characters from a user.
    ///
    /// Updates the user's typing session state (position, speed, accuracy),
    /// saves the updated session to the cache, and broadcasts the progress
    /// to all participants in the tournament room.
    ///
    /// # Arguments
    ///
    /// * `socket` - The user's socket connection reference.
    /// * `typed_chars` - A vector of characters typed by the user since the last update.
    pub async fn handle_typing(self: Self, socket: SocketRef, typed_chars: Vec<char>) {
        let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();

        if typed_chars.is_empty() {
            warn!(client_id = %client.id, "Received empty typing event. Ignoring.");
            return;
        }

        let typing_text = self.inner.typing_text.clone();
        let cache = self.inner.participants.clone();

        let typing_session = match cache.get_data(&client.id) {
            Some(session) => session,
            None => {
                warn!(client_id = %client.id, "Typing event received, but no active session found.");
                let failure_payload = WsFailurePayload::new(2210, "Client ID not found.");
                socket.emit("type:failure", &failure_payload).ok();
                return;
            }
        };

        let challenge_text_bytes = typing_text.as_bytes();

        // --- Process Input and Update State ---
        let now = Utc::now();
        let updated_session =
            process_typing_input(typing_session, typed_chars, challenge_text_bytes, now);

        cache.set_data(&updated_session.client.id, updated_session.clone());

        let changes = PartialParticipantData {
            current_position: Some(updated_session.current_position),
            correct_position: Some(updated_session.correct_position),
            total_keystrokes: Some(updated_session.total_keystrokes),
            current_speed: Some(updated_session.current_speed),
            current_accuracy: Some(updated_session.current_accuracy),
            started_at: updated_session.started_at,
            ended_at: updated_session.ended_at,
        };

        let update_me_payload = UpdateMePayload {
            updates: changes.clone(),
        };

        if let Err(e) = socket.emit("update:me", &update_me_payload) {
            warn!("Failed to send update:me to {}: {}", client.id, e);
        }

        self.inner.update_all_notifier.notify_one();
    }

    fn register_socket_listeners(self: Self, socket: SocketRef, spectator: bool) {
        let client = socket
            .req_parts()
            .extensions
            .get::<ClientSchema>()
            .expect("ClientSchema not found in socket extensions during listener registration")
            .clone();

        if !spectator {
            // wait period before processing a new character
            let debounce_duration = DEBOUNCE_DURATION;
            // user should only experience at worst 3s lag time
            // but will likely be in millis under normal circumstances
            let max_process_wait = MAX_PROCESS_WAIT;
            // processing shouldn't lag behind by more than 15 chars from current position
            // but will likely be instantaneous under normal circumstances
            let max_process_stack_size = MAX_PROCESS_STACK_SIZE;
            let cleanup_wait_duration = INACTIVITY_TIMEOUT_DURATION;
            let client = client.clone();
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

            let frequency_monitor = Arc::new(FrequencyMonitor::new(
                debounce_duration,
                max_process_wait,
                max_process_stack_size,
            ));

            socket.on("type", {
                let frequency_monitor = frequency_monitor.clone();
                let timeout_monitor = timeout_monitor.clone();
                let manager_clone = self.clone();
                async move |socket: SocketRef, Data::<TypeEventPayload>(TypeEventPayload { character })| {
                    let processor = async move {
                        frequency_monitor
                            .call(character, move |chars: Vec<char>| {
                                Self::handle_typing(manager_clone, socket, chars)
                            })
                            .await;
                    };

                    timeout_monitor.call(processor).await;
                }
            });
        }

        socket.on("check", {
            let manager_clone_check = self.clone(); // Clone manager for the async block
            move |s: SocketRef| {
                let mc_check = manager_clone_check.clone();
                let socket_check = s.clone();
                async move {
                    info!(
                        "Client {} requesting tournament status check for {}",
                        socket_check.id, mc_check.inner.tournament_id
                    );

                    let status = {
                        let session_state_guard =
                            mc_check.inner.tournament_session_state.lock().await;

                        if session_state_guard.ended_at.is_some() {
                            "ENDED"
                        } else if session_state_guard.started_at.is_some() {
                            "STARTED"
                        } else {
                            "UPCOMING"
                        }
                    };

                    let success_payload = serde_json::json! ({ "status": status });

                    if socket_check
                        .emit("check:success", &success_payload)
                        .is_err()
                    {
                        warn!(
                            "Failed to send check:success to client {} for tournament {}",
                            socket_check.id, mc_check.inner.tournament_id
                        );
                    }
                }
            }
        });

        socket.on("leave", {
            let manager_clone_leave = self.clone();
            let client_leave = client.clone();
            move |s: SocketRef| {
                let mc_leave = manager_clone_leave.clone();
                let cid_leave = client_leave.id.clone();
                let socket_leave = s.clone();
                async move {
                    info!(
                        "Client {} is attempting to leave tournament {}",
                        cid_leave, mc_leave.inner.tournament_id
                    );
                    if !spectator {
                        mc_leave
                            .handle_participant_leave(&cid_leave, &socket_leave)
                            .await
                            .map_err(|e| {
                                warn!(
                                    "Error during leave handling for client {}: {}",
                                    cid_leave, e
                                );
                            })
                            .ok();
                    }
                }
            }
        });

        socket.on_disconnect({
            let manager_clone_disconnect = self.clone();
            move |s: SocketRef| {
                let mc_disconnect = manager_clone_disconnect.clone();
                let client = s
                    .req_parts()
                    .extensions
                    .get::<ClientSchema>()
                    .expect("ClientSchema not found in socket extensions during disconnect")
                    .clone();
                async move {
                    info!(
                        "Client {} disconnected from tournament {}",
                        client.id, mc_disconnect.inner.tournament_id
                    );
                }
            }
        });

        if !spectator {
            socket.on("me", {
                let manager_clone_me = self.clone();
                move |s: SocketRef| {
                    let mc_me = manager_clone_me.clone();
                    let client_me =
                        s.req_parts().extensions.get::<ClientSchema>().expect(
                            "ClientSchema not found in socket extensions during disconnect",
                        );
                    let cid_me = client_me.id.clone();
                    let socket_me = s.clone();
                    async move {
                        if let Some(session_data) = mc_me.inner.participants.get_data(&cid_me) {
                            let participant_data =
                                Self::map_session_to_api_participant_data(&session_data);
                            if socket_me.emit("me:success", &participant_data).is_err() {
                                warn!("Failed to send me:success to client {}", cid_me);
                            }
                        } else {
                            let failure_payload =
                                WsFailurePayload::new(3101, "Your session was not found.");
                            if socket_me.emit("me:failure", &failure_payload).is_err() {
                                warn!("Failed to send me:failure to client {}", cid_me);
                            }
                        }
                    }
                }
            });
        }

        socket.on("all", {
            let manager_clone_all = self.clone();
            move |s: SocketRef| {
                let mc_all = manager_clone_all.clone();
                let socket_all = s.clone();
                async move {
                    let all_participants_api_data: Vec<ParticipantData> = mc_all
                        .inner
                        .participants
                        .values()
                        .iter()
                        .map(|session| Self::map_session_to_api_participant_data(session))
                        .collect();
                    if socket_all
                        .emit("all:success", &all_participants_api_data)
                        .is_err()
                    {
                        warn!("Failed to send all:success to client");
                    }
                }
            }
        });

        socket.on("data", {
            let manager_clone_data = self.clone();
            move |s: SocketRef| {
                let mc_data = manager_clone_data.clone();
                let socket_data = s.clone();
                async move {
                    let current_tournament_data;
                    {
                        let t_session_state_guard =
                            mc_data.inner.tournament_session_state.lock().await;
                        let t_meta = &mc_data.inner.tournament_meta;
                        current_tournament_data = TournamentData {
                            id: t_meta.id.clone(),
                            title: t_meta.title.clone(),
                            created_at: t_meta.created_at,
                            created_by: t_meta.created_by.clone(),
                            scheduled_for: t_meta.scheduled_for,
                            description: t_meta.description.clone(),
                            started_at: t_session_state_guard.started_at,
                            ended_at: t_session_state_guard.ended_at,
                            text: if t_session_state_guard.started_at.is_some() {
                                Some(mc_data.inner.typing_text.to_string())
                            } else {
                                None
                            },
                        };
                    }
                    if socket_data
                        .emit("data:success", &current_tournament_data)
                        .is_err()
                    {
                        // Emitting specific "data:success"
                        warn!("Failed to send data:success to client");
                    }
                }
            }
        });
    }

    async fn handle_participant_leave(
        self: &Self,
        client_id_str: &str,
        socket: &SocketRef,
    ) -> Result<()> {
        info!(
            "Handling leave for client {} in tournament {}",
            client_id_str, self.inner.tournament_id
        );

        if self.inner.participants.delete_data(client_id_str).is_some() {
            self.inner.session_registry.delete_session(client_id_str);

            socket.leave(self.inner.tournament_id.to_string());

            let member_left_payload = MemberLeftPayload {
                client_id: client_id_str.to_string(),
            };

            let io_clone = self.inner.io.clone();
            let tournament_id_str = self.inner.tournament_id.to_string();

            if let Err(e) = io_clone
                .to(tournament_id_str.clone())
                .except(socket.id)
                .emit("member:left", &member_left_payload)
                .await
            {
                warn!(
                    "Failed to broadcast member:left for {}: {}",
                    client_id_str, e
                );
            }

            let leave_success_payload = LeaveSuccessPayload {
                message: "Left tournament successfully".to_string(),
            };
            if socket
                .emit("leave:success", &leave_success_payload)
                .is_err()
            {
                warn!(
                    "Failed to send leave:success to {}: {}",
                    client_id_str, socket.id
                );
            }

            if self.inner.participants.count() == 0 {
                let session_ended;
                {
                    let session_state = self.inner.tournament_session_state.lock().await;
                    session_ended = session_state.ended_at.is_some();
                }
                if !session_ended {
                    info!(
                        "Last participant left tournament {}. Cleaning up.",
                        self.inner.tournament_id
                    );
                    // Self::cleanup(self.inner.tournament_registry.clone(), &self.inner.tournament_id);
                    // Note: Automatic cleanup on last leave might be aggressive.
                    // Consider if an empty tournament should persist until its scheduled end or manual cleanup.
                    // For now, commenting out aggressive cleanup.
                }
            }
            Ok(())
        } else {
            warn!(
                "Leave/disconnect for client {} but no session found in tournament {}.",
                client_id_str, self.inner.tournament_id
            );
            let failure_payload =
                WsFailurePayload::new(2210, "You are not currently in this tournament session.");
            if socket.emit("leave:failure", &failure_payload).is_err() {
                warn!(
                    "Failed to send leave:failure to {}: {}",
                    client_id_str, socket.id
                );
            }
            Err(anyhow::anyhow!("Client session not found for leave"))
        }
    }

    // broadcast_tournament_update is removed as updates are now granular:
    // join:success, member:joined, member:left, update:me, update:all, update:data

    pub fn cleanup(tournament_registry: TournamentRegistry, tournament_id: &Arc<String>) {
        // Takes Arc<String>
        info!("Cleaning up manager for tournament {}", tournament_id);
        tournament_registry.evict(tournament_id.as_str()); // Evict takes &str
    }
}

/// Handles the automatic leaving of a user due to inactivity timeout.
///
/// Retrieves the user's current tournament (if any) and calls `handle_leave`.
///
/// # Arguments
///
/// * `client` - The client information of the user who timed out.
/// * `socket` - The user's socket connection reference.
pub async fn handle_timeout(client: &ClientSchema, _socket: SocketRef) {
    info!(client_id = %client.id, "Handling inactivity timeout");
}

fn process_typing_input(
    mut session: TypingSessionSchema,
    typed_chars: Vec<char>,
    challenge_text: &[u8],
    now: DateTime<Utc>,
) -> TypingSessionSchema {
    if session.started_at.is_none() {
        session.started_at = Some(now);
    }

    let text_len = challenge_text.len();

    for current_char in typed_chars {
        if session.correct_position >= text_len && session.ended_at.is_some() {
            warn!(user_id=%session.client.id, "Received typing input after session ended. Ignoring.");
            break;
        }

        if current_char == '\u{8}' {
            // Backspace character (`\b` or unicode backspace)
            if session.current_position > session.correct_position {
                session.current_position -= 1;
            } else if session.current_position == session.correct_position
                && session.current_position > 0
            {
                if challenge_text[session.current_position - 1] != b' ' {
                    session.correct_position -= 1;
                    session.current_position -= 1;
                }
            }
            // If current_position is 0, backspace does nothing.
            // No change to total_keystrokes for backspace.
        } else {
            session.total_keystrokes += 1;

            if session.current_position < text_len {
                let expected_char = challenge_text[session.current_position];
                if session.current_position == session.correct_position
                    && (current_char as u32) == (expected_char as u32)
                {
                    session.correct_position += 1;
                }
                session.current_position += 1;
            }
        }

        if session.correct_position == text_len && session.ended_at.is_none() {
            session.ended_at = Some(now);
            session.current_position = session.correct_position;
            info!(client_id = %session.client.id, tournament_id = %session.tournament_id, "User finished typing challenge");
            break;
        }
    }

    if let Some(started_at) = session.started_at {
        let end_time = session.ended_at.unwrap_or(now);
        let duration = end_time.signed_duration_since(started_at);

        let minutes_elapsed = (duration.num_milliseconds() as f32 / 60000.0).max(0.0001);

        session.current_speed = (session.correct_position as f32 / 5.0 / minutes_elapsed).round();

        session.current_accuracy = if session.total_keystrokes > 0 {
            ((session.correct_position as f32 / session.total_keystrokes as f32) * 100.0)
                .round()
                .clamp(0.0, 100.0)
        } else {
            100.0
        };
    } else {
        session.current_speed = 0.0;
        session.current_accuracy = 100.0;
    }

    session
}
