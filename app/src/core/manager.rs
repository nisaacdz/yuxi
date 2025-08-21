use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use models::{
    params::tournament::UpdateTournamentParams,
    schemas::{
        tournament::{TournamentLiveData, TournamentSchema, TournamentSession},
        typing::{TournamentStatus, TypingSessionSchema},
        user::TournamentRoomMember,
    },
};
use serde::Serialize;
use socketioxide::extract::{Data, SocketRef};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::{
    cache::Cache,
    core::{
        debouncer::{Debouncer, DebouncerConfig},
        moderation::FrequencyMonitor,
        timeout::TimeoutMonitor,
    },
    persistence::{text::generate_text, tournaments::update_tournament},
    state::AppState,
};

const JOIN_DEADLINE: Duration = Duration::from_secs(15);
const INACTIVITY_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

const DEBOUNCE_DURATION: Duration = Duration::from_millis(50);
const MAX_PROCESS_WAIT: Duration = Duration::from_millis(500);
const MAX_PROCESS_STACK_SIZE: usize = 3;

const UPDATE_ALL_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);
const UPDATE_ALL_MAX_STACK_SIZE: usize = 15;
const UPDATE_ALL_MAX_WAIT: Duration = Duration::from_secs(5);

#[derive(Serialize, Debug, Clone)]
pub struct WsFailurePayload {
    code: i32,
    message: String,
}

impl WsFailurePayload {
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code: code,
            message: message.to_string(),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ParticipantData {
    member: TournamentRoomMember,
    current_position: usize,
    correct_position: usize,
    total_keystrokes: i32,
    current_speed: f32,
    current_accuracy: f32,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, Clone, Copy)]
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
struct PartialParticipantDataForUpdate<'a> {
    member_id: &'a str,
    updates: PartialParticipantData,
}

#[derive(Serialize, Debug, Clone)]
struct UpdateMePayload {
    updates: PartialParticipantData,
}

#[derive(Serialize, Debug, Clone)]
struct UpdateAllPayload<'a> {
    updates: Vec<PartialParticipantDataForUpdate<'a>>,
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
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    scheduled_end: Option<DateTime<Utc>>,
    text: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PartialTournamentData {
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
    text: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct JoinSuccessPayload {
    data: TournamentData,
    member: TournamentRoomMember,
    participants: Vec<ParticipantData>,
    noauth: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ParticipantJoinedPayload {
    participant: ParticipantData,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ParticipantLeftPayload {
    member_id: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LeaveSuccessPayload {
    message: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateDataPayload {
    updates: PartialTournamentData,
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
    app_state: AppState,
    typing_text: RwLock<Arc<String>>,
}

impl TournamentManagerInner {
    async fn broadcast_update_data(self: &Arc<Self>, start: bool) {
        let update_data_payload = {
            let (started_at, ended_at) = {
                let lock = self.tournament_session_state.lock().await;
                (lock.started_at, lock.ended_at)
            };

            UpdateDataPayload {
                updates: PartialTournamentData {
                    title: None,
                    scheduled_for: None,
                    description: None,
                    started_at: if start { started_at } else { None },
                    ended_at: ended_at,
                    text: if start {
                        Some(self.typing_text.read().unwrap().to_string())
                    } else {
                        None
                    },
                },
            }
        };
        let tournament_id = self.tournament_id.clone();
        let io_clone = self.app_state.socket_io.clone();

        io_clone
            .to(tournament_id.to_string())
            .emit("update:data", &update_data_payload)
            .await
            .inspect_err(|e| error!("Failed to emit update:data for tournament start: {}", e))
            .ok();
    }
}

#[derive(Clone)]
pub struct TournamentManager {
    inner: Arc<TournamentManagerInner>,
    update_all_broadcaster: Debouncer,
}

impl TournamentManager {
    pub fn new(tournament_schema: TournamentSchema, app_state: AppState) -> Self {
        info!(
            "Initializing TournamentManager for {}",
            &tournament_schema.id
        );

        let initial_session_state = Mutex::new(TournamentSession::new(
            tournament_schema.id.clone(),
            tournament_schema.scheduled_for,
            None,
        ));

        let participants = Cache::new();

        let tournament_id_arc = Arc::new(tournament_schema.id.to_string());
        let typing_text_arc = Arc::new("".to_string());

        let inner_manager_state = Arc::new(TournamentManagerInner {
            tournament_id: tournament_id_arc.clone(),
            tournament_meta: Arc::new(tournament_schema.clone()),
            tournament_session_state: initial_session_state,
            participants,
            app_state: app_state.clone(),
            typing_text: RwLock::new(typing_text_arc),
        });

        let update_all_broadcaster =
            Self::create_update_all_broadcaster(inner_manager_state.clone());

        let manager = Self {
            inner: inner_manager_state,
            update_all_broadcaster: update_all_broadcaster.clone(),
        };

        {
            let manager = manager.clone();
            let tournament_id = tournament_schema.id.clone();
            let scheduled_for = tournament_schema.scheduled_for;

            crate::scheduler::schedule_new_task(
                async move {
                    manager.execute_tournament_start_logic().await;
                },
                scheduled_for,
            )
            .inspect_err(|e| {
                error!(
                    "Failed to schedule start task for tournament {}: {}",
                    tournament_id, e
                );
            })
            .ok();
        }

        manager
    }

    fn create_update_all_broadcaster(inner: Arc<TournamentManagerInner>) -> Debouncer {
        Debouncer::new(
            move || {
                let inner = inner.clone();
                tokio::task::spawn(async move {
                    let all_participants = inner.participants.values();

                    if all_participants.iter().filter_map(|v| v.ended_at).count()
                        == all_participants.len()
                    {
                        if let Some(manager) = inner
                            .app_state
                            .tournament_registry
                            .get(&inner.tournament_id)
                        {
                            manager.shutdown().await;
                        } else {
                            error!("Tournament manager not found for {}", inner.tournament_id);
                        }
                        //inner.end_tournament().await;
                    }

                    let updates_for_all = all_participants
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
                                member_id: &session_data.member.id,
                                updates: partial_data,
                            }
                        })
                        .collect::<Vec<_>>();

                    if updates_for_all.is_empty() {
                        return;
                    }

                    let update_all_payload = UpdateAllPayload {
                        updates: updates_for_all,
                    };

                    let tournament_room_id = inner.tournament_id.to_string();
                    let io_clone = inner.app_state.socket_io.clone();

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
                });
            },
            DebouncerConfig {
                debounce_duration: UPDATE_ALL_DEBOUNCE_DURATION,
                max_stack_size: UPDATE_ALL_MAX_STACK_SIZE,
                max_debounce_period: UPDATE_ALL_MAX_WAIT,
            },
        )
    }

    async fn execute_tournament_start_logic(self: Self) {
        let participant_count = self.inner.participants.count();

        if participant_count > 0 {
            let text = generate_text(self.inner.tournament_meta.text_options.unwrap_or_default());
            let mut session_state_guard = self.inner.tournament_session_state.lock().await;
            let current_time = Utc::now();
            let scheduled_end = current_time + TimeDelta::minutes(10);
            session_state_guard.scheduled_end = Some(scheduled_end);
            *self.inner.typing_text.write().unwrap() = Arc::new(text);
            session_state_guard.started_at = Some(current_time);
            std::mem::drop(session_state_guard);

            for socket in self
                .inner
                .app_state
                .socket_io
                .within(self.inner.tournament_id.to_string())
                .sockets()
            {
                self.register_type_listener(socket);
            }

            self.inner.broadcast_update_data(true).await;
            let manager = self.clone();

            crate::scheduler::schedule_new_task(
                async move {
                    manager.shutdown().await;
                    // manager.inner.end_tournament().await;
                    // manager.update_all_broadcaster.shutdown().await;
                },
                scheduled_end,
            )
            .ok();
        } else {
            info!(
                "No participants for tournament {}. Ending immediately.",
                self.inner.tournament_id
            );
            self.shutdown().await;
            //self.inner.end_tournament().await;
        }
    }

    fn map_session_to_api_participant_data(session: &TypingSessionSchema) -> ParticipantData {
        ParticipantData {
            member: session.member.clone(),
            current_position: session.current_position,
            correct_position: session.correct_position,
            total_keystrokes: session.total_keystrokes,
            current_speed: session.current_speed,
            current_accuracy: session.current_accuracy,
            started_at: session.started_at,
            ended_at: session.ended_at,
        }
    }

    pub async fn connect(
        self: Self,
        socket: SocketRef,
        spectator: bool,
        noauth: String,
    ) -> Result<()> {
        let member_schema = socket.extensions.get::<TournamentRoomMember>().unwrap();

        let now = Utc::now();

        if !spectator && !self.inner.participants.contains_key(&member_schema.id) {
            let (started_at, ended_at) = {
                let session_state_guard = self.inner.tournament_session_state.lock().await;
                (session_state_guard.started_at, session_state_guard.ended_at)
            };

            let scheduled_for = self.inner.tournament_meta.scheduled_for;

            if ended_at.is_some()
                || started_at.is_some()
                || (scheduled_for - now < TimeDelta::from_std(JOIN_DEADLINE).unwrap())
            {
                error!(member_id = %member_schema.id, "Tournament no longer accepting participants.");
                let failure_payload =
                    WsFailurePayload::new(1004, "Tournament no longer accepting participants.");

                if socket.emit("join:failure", &failure_payload).is_err() {
                    warn!("Failed to send join:failure to member {}", member_schema.id);
                }
                return Err(anyhow::anyhow!(
                    "Tournament no longer accepting participants."
                ));
            }
        }

        info!(
            "Handling connection for member {} to tournament {}",
            &member_schema.id, self.inner.tournament_id
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
                    Some(self.inner.typing_text.read().unwrap().to_string())
                } else {
                    None
                },
                scheduled_end: t_session_state_guard.scheduled_end,
            };
        }

        if !spectator {
            // Add or get participant session
            let participant_session =
                self.inner
                    .participants
                    .get_or_insert(&member_schema.id, || {
                        TypingSessionSchema::new(
                            member_schema.clone(),
                            self.inner.tournament_id.to_string(),
                        )
                    });
            // Update the global session registry
            self.inner
                .app_state
                .typing_session_registry
                .set_session(&member_schema.id, participant_session.clone());

            // Broadcast "participant:joined" to other members in the room
            let new_participant_api_data =
                Self::map_session_to_api_participant_data(&participant_session);
            let participant_joined_payload = ParticipantJoinedPayload {
                participant: new_participant_api_data,
            };

            let io_clone = self.inner.app_state.socket_io.clone();
            let tournament_id_str = self.inner.tournament_id.to_string();

            if let Err(e) = io_clone
                .to(tournament_id_str)
                .except(socket.id)
                .emit("participant:joined", &participant_joined_payload)
                .await
            {
                warn!("Failed to broadcast participant:joined: {}", e);
            }
        }

        let all_participants_api_data = self
            .inner
            .participants
            .values()
            .iter()
            .map(|s| Self::map_session_to_api_participant_data(s))
            .collect::<Vec<_>>();

        let join_success_payload = JoinSuccessPayload {
            data: current_tournament_data,
            member: member_schema.clone(),
            participants: all_participants_api_data,
            noauth,
        };

        // Emit join:success to the current socket
        if socket.emit("join:success", &join_success_payload).is_err() {
            warn!("Failed to send join:success to member {}", member_schema.id);
        }

        // Register other event listeners for this socket
        self.clone()
            .register_base_listeners(socket.clone(), spectator);

        info!(
            "Member {} connected to tournament {}",
            &member_schema.id, self.inner.tournament_id
        );

        Ok(())
    }

    pub async fn handle_typing(self: Self, socket: SocketRef, typed_chars: Vec<char>) {
        let member = socket.extensions.get::<TournamentRoomMember>().unwrap();

        if typed_chars.is_empty() {
            warn!(member_id = %member.id, "Received empty typing event. Ignoring.");
            return;
        }

        let cache = self.inner.participants.clone();

        let typing_session = match cache.get_data(&member.id) {
            Some(session) => session,
            None => {
                warn!(member_id = %member.id, "Typing event received, but no active session found.");
                let failure_payload = WsFailurePayload::new(2210, "Member ID not found.");
                socket.emit("type:failure", &failure_payload).ok();
                return;
            }
        };

        if typing_session.ended_at.is_some() {
            warn!(member_id = %typing_session.member.id, "Received typing input after session ended. Ignoring.");
            let failure_payload = WsFailurePayload::new(2211, "Your session has ended.");
            socket.emit("type:failure", &failure_payload).ok();
            return;
        }

        let typing_text = self.inner.typing_text.read().unwrap().clone();

        let challenge_text_bytes = typing_text.as_bytes();

        // --- Process Input and Update State ---
        let now = Utc::now();
        let updated_session =
            process_typing_input(typing_session, typed_chars, challenge_text_bytes, now);

        cache.set_data(&updated_session.member.id, updated_session.clone());

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
            warn!("Failed to send update:me to {}: {}", member.id, e);
        }

        self.update_all_broadcaster.trigger();
    }

    fn register_type_listener(self: &Self, socket: SocketRef) {
        let member = socket.extensions.get::<TournamentRoomMember>().unwrap();

        if member.participant {
            let debounce_duration = DEBOUNCE_DURATION;
            let max_process_wait = MAX_PROCESS_WAIT;
            let max_process_stack_size = MAX_PROCESS_STACK_SIZE;
            let cleanup_wait_duration = INACTIVITY_TIMEOUT_DURATION;
            let manager_clone = self.clone();
            let timeout_monitor = {
                let socket = socket.clone();
                let manager_clone = manager_clone.clone();

                let after_timeout_fn = { async move || info!("Timedout user now typing") };

                Arc::new(TimeoutMonitor::new(
                    async move || {
                        manager_clone.handle_timeout(socket).await;
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
    }

    fn register_base_listeners(self: Self, socket: SocketRef, spectator: bool) {
        let member = socket.extensions.get::<TournamentRoomMember>().unwrap();

        socket.on("check", {
            let manager_clone_check = self.clone(); // Clone manager for the async block
            move |s: SocketRef| {
                let mc_check = manager_clone_check.clone();
                let socket_check = s.clone();
                async move {
                    info!(
                        "Member {} requesting tournament status check for {}",
                        socket_check.id, mc_check.inner.tournament_id
                    );

                    let status = {
                        let session_state_guard =
                            mc_check.inner.tournament_session_state.lock().await;

                        if session_state_guard.ended_at.is_some() {
                            TournamentStatus::Ended
                        } else if session_state_guard.started_at.is_some() {
                            TournamentStatus::Started
                        } else {
                            TournamentStatus::Upcoming
                        }
                    };

                    let success_payload = serde_json::json! ({ "status": status });

                    if socket_check
                        .emit("check:success", &success_payload)
                        .is_err()
                    {
                        warn!(
                            "Failed to send check:success to member {} for tournament {}",
                            socket_check.id, mc_check.inner.tournament_id
                        );
                    }
                }
            }
        });

        socket.on("leave", {
            let manager_clone_leave = self.clone();
            let member_leave = member.clone();
            move |s: SocketRef| {
                let mc_leave = manager_clone_leave.clone();
                let cid_leave = member_leave.id.clone();
                let socket_leave = s.clone();
                async move {
                    info!(
                        "Member {} is attempting to leave tournament {}",
                        cid_leave, mc_leave.inner.tournament_id
                    );
                    if !spectator {
                        mc_leave
                            .handle_participant_leave(&cid_leave, &socket_leave)
                            .await
                            .map_err(|e| {
                                warn!(
                                    "Error during leave handling for member {}: {}",
                                    cid_leave, e
                                );
                            })
                            .ok();
                    }
                    let leave_success_payload = LeaveSuccessPayload {
                        message: "Left tournament successfully".to_string(),
                    };
                    if s.emit("leave:success", &leave_success_payload).is_err() {
                        warn!("Failed to send leave:success to {}: {}", cid_leave, s.id);
                    }
                }
            }
        });

        socket.on_disconnect({
            let manager_clone_disconnect = self.clone();
            move |s: SocketRef| {
                let mc_disconnect = manager_clone_disconnect.clone();
                let member = s.extensions.get::<TournamentRoomMember>().unwrap();
                async move {
                    info!(
                        "Member {} disconnected from tournament {}",
                        member.id, mc_disconnect.inner.tournament_id
                    );
                }
            }
        });

        if !spectator {
            socket.on("me", {
                let manager_clone_me = self.clone();
                move |s: SocketRef| {
                    let mc_me = manager_clone_me.clone();
                    let member_me = s.extensions.get::<TournamentRoomMember>().unwrap();
                    let cid_me = member_me.id;
                    let socket_me = s.clone();
                    async move {
                        if let Some(session_data) = mc_me.inner.participants.get_data(&cid_me) {
                            let participant_data =
                                Self::map_session_to_api_participant_data(&session_data);
                            if socket_me.emit("me:success", &participant_data).is_err() {
                                warn!("Failed to send me:success to member {}", cid_me);
                            }
                        } else {
                            let failure_payload =
                                WsFailurePayload::new(3101, "Your session was not found.");
                            if socket_me.emit("me:failure", &failure_payload).is_err() {
                                warn!("Failed to send me:failure to member {}", cid_me);
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
                        warn!("Failed to send all:success to member");
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
                                Some(mc_data.inner.typing_text.read().unwrap().to_string())
                            } else {
                                None
                            },
                            scheduled_end: t_session_state_guard.scheduled_end,
                        };
                    }
                    if socket_data
                        .emit("data:success", &current_tournament_data)
                        .is_err()
                    {
                        // Emitting specific "data:success"
                        warn!("Failed to send data:success to member");
                    }
                }
            }
        });
    }

    async fn handle_participant_leave(
        self: &Self,
        member_id_str: &str,
        socket: &SocketRef,
    ) -> Result<()> {
        info!(
            "Handling leave for member {} in tournament {}",
            member_id_str, self.inner.tournament_id
        );

        if self.inner.participants.delete_data(member_id_str).is_some() {
            self.inner
                .app_state
                .typing_session_registry
                .delete_session(member_id_str);

            socket.leave(self.inner.tournament_id.to_string());

            let participant_left_payload = ParticipantLeftPayload {
                member_id: member_id_str.to_string(),
            };

            let io_clone = self.inner.app_state.socket_io.clone();
            let tournament_id_str = self.inner.tournament_id.to_string();

            if let Err(e) = io_clone
                .to(tournament_id_str.clone())
                .except(socket.id)
                .emit("participant:left", &participant_left_payload)
                .await
            {
                warn!(
                    "Failed to broadcast participant:left for {}: {}",
                    member_id_str, e
                );
            }

            if self.inner.participants.count() == 0 {
                let started = {
                    self.inner
                        .tournament_session_state
                        .lock()
                        .await
                        .started_at
                        .is_some()
                };
                if started {
                    self.shutdown().await;
                    //self.inner.end_tournament().await;
                }
            }
            Ok(())
        } else {
            warn!(
                "Leave/disconnect for member {} but no session found in tournament {}.",
                member_id_str, self.inner.tournament_id
            );

            Err(anyhow::anyhow!("Member session not found for leave"))
        }
    }

    pub async fn handle_timeout(self: Self, socket: SocketRef) {
        let member = socket.extensions.get::<TournamentRoomMember>().unwrap();
        self.inner.participants.update_data(&member.id, |m| {
            m.ended_at = Some(Utc::now());
        });
        self.update_all_broadcaster.trigger();
    }

    pub async fn live_data(&self, member_id: &str) -> TournamentLiveData {
        let participant_count = self.inner.participants.count();
        let participating = self.inner.participants.contains_key(&member_id);

        let (started_at, ended_at) = {
            let session_state_guard = self.inner.tournament_session_state.lock().await;
            (session_state_guard.started_at, session_state_guard.ended_at)
        };

        TournamentLiveData {
            participant_count,
            participating,
            started_at,
            ended_at,
        }
    }

    pub async fn shutdown(self: &Self) {
        let mut session_data = self.inner.tournament_session_state.lock().await;

        // 1. Idempotency Check: If already ending/ended, do nothing.
        if session_data.ended_at.is_some() {
            return;
        }

        // 2. Mark as Ended
        let now = Utc::now();
        session_data.ended_at = Some(now);
        std::mem::drop(session_data); // Release lock

        info!(
            "Shutting down manager for tournament {}",
            &*self.inner.tournament_id
        );

        // 3. Persist Final State to Database
        if let Err(e) = update_tournament(
            &self.inner.app_state,
            UpdateTournamentParams {
                id: Some(self.inner.tournament_id.to_string()),
                ended_at: Some(Some(now.fixed_offset())),
                ..Default::default()
            },
        )
        .await
        {
            error!("Failed to persist final tournament state: {}", e);
        }

        // 4. Broadcast Final Update
        self.inner.broadcast_update_data(false).await;

        // 6. Shutdown Internal Components
        self.update_all_broadcaster.shutdown().await;

        // 7. Schedule Final Eviction of the Manager
        let manager_clone = self.clone();
        let evict_task = async move {
            // 5. Clean Up All Associated State (Fixing the Gap)
            let participant_ids: Vec<String> = manager_clone.inner.participants.keys();
            for member_id in participant_ids {
                manager_clone.inner.participants.delete_data(&member_id);
                manager_clone
                    .inner
                    .app_state
                    .typing_session_registry
                    .delete_session(&member_id);
            }
            info!(
                "Cleaned up {} participant sessions from registries.",
                manager_clone.inner.participants.count()
            );

            manager_clone
                .inner
                .app_state
                .tournament_registry
                .evict(&manager_clone.inner.tournament_id);
            info!(
                "Evicted TournamentManager for {}",
                &*manager_clone.inner.tournament_id
            );
        };

        // The 10-minute grace period remains.
        let evict_on = Utc::now() + TimeDelta::minutes(10);
        crate::scheduler::schedule_new_task(evict_task, evict_on).ok();
    }
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
        if session.correct_position >= text_len {
            warn!(user_id=%session.member.id, "Received typing input after session ended. Ignoring.");
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
            info!(member_id = %session.member.id, tournament_id = %session.tournament_id, "User finished typing challenge");
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
