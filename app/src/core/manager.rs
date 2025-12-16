use crate::core::{algorithm::*, dtos::*};
use anyhow::Result;
use chrono::{TimeDelta, Utc};
use models::{
    params::tournament::UpdateTournamentParams,
    schemas::{
        tournament::{TournamentLiveData, TournamentSchema, TournamentSession},
        typing::{TournamentStatus, TypingSessionSchema},
        user::TournamentRoomMember,
    },
};
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

const DEBOUNCE_DURATION: Duration = Duration::from_millis(250);
const MAX_PROCESS_WAIT: Duration = Duration::from_millis(800);
const MAX_PROCESS_STACK_SIZE: usize = 5;

const UPDATE_ALL_DEBOUNCE_DURATION: Duration = Duration::from_millis(1000);
const UPDATE_ALL_MAX_STACK_SIZE: usize = 20;
const UPDATE_ALL_MAX_WAIT: Duration = Duration::from_secs(3);

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
                    ended_at,
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
    algorithm: Arc<dyn TypingAlgorithm + Sync + Send>,
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
            algorithm: Arc::new(ZeroProceed),
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
                            tokio::spawn(async move {
                                manager.shutdown().await;
                            });
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

    async fn execute_tournament_start_logic(self) {
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
                self.register_type_listeners(socket, false);
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

    pub async fn connect(self, socket: SocketRef, spectator: bool, noauth: String) -> Result<()> {
        let member_schema = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();

        let now = Utc::now();

        if !spectator && !self.inner.participants.contains_key(&member_schema.id) {
            let (started_at, ended_at) = {
                let session_state_guard = self.inner.tournament_session_state.lock().await;
                (session_state_guard.started_at, session_state_guard.ended_at)
            };

            let scheduled_for = self.inner.tournament_meta.scheduled_for;
            let join_deadline =
                TimeDelta::from_std(JOIN_DEADLINE).unwrap_or_else(|_| TimeDelta::seconds(15));

            if ended_at.is_some() || started_at.is_some() || (scheduled_for - now < join_deadline) {
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
                            (*member_schema).clone(),
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
            member: (*member_schema).clone(),
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

    async fn handle_progress(self, socket: SocketRef, progress: ProgressEventPayload) {
        let member = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();

        let rid = progress.rid;

        let original = self.inner.typing_text.read().unwrap().clone();

        let update_result = self.inner.participants.update_data(
            &member.id,
            // This closure contains all the mutation logic.
            // It receives `&mut TypingSessionSchema`.
            move |session| {
                self.algorithm
                    .handle_progress(session, progress, original.as_bytes())
            },
        );

        match update_result {
            Some(Ok(changes)) => {
                let update_me_payload = UpdateMePayload {
                    updates: changes,
                    rid,
                };
                if let Err(e) = socket.emit("update:me", &update_me_payload) {
                    warn!("Failed to send update:me to {}: {}", member.id, e);
                }
                self.update_all_broadcaster.trigger();
            }

            Some(Err(failure_payload)) => {
                warn!(member_id = %member.id, "Progress update failed: {}", failure_payload.message);
                socket.emit("progress:failure", &failure_payload).ok();
            }

            None => {
                warn!(member_id = %member.id, "Progress event received, but no active session found.");
                let failure_payload = WsFailurePayload::new(2210, "Member ID not found.");
                socket.emit("progress:failure", &failure_payload).ok();
            }
        }
    }

    async fn handle_typing(self, socket: SocketRef, typed_chars: Vec<char>, rid: i32) {
        let member = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();
        let cache = self.inner.participants.clone();
        let original = self.inner.typing_text.read().unwrap().clone();

        let update_result = cache.update_data(&member.id, move |session| {
            self.algorithm
                .handle_type(session, &typed_chars, original.as_bytes())
        });

        match update_result {
            Some(Ok(changes)) => {
                let update_me_payload = UpdateMePayload {
                    updates: changes,
                    rid,
                };
                if let Err(e) = socket.emit("update:me", &update_me_payload) {
                    warn!("Failed to send update:me to {}: {}", member.id, e);
                }
                self.update_all_broadcaster.trigger();
            }

            Some(Err(failure_payload)) => {
                warn!(member_id = %member.id, "Type event failed: {}", failure_payload.message);
                socket.emit("type:failure", &failure_payload).ok();
            }

            None => {
                warn!(member_id = %member.id, "Type event received, but no active session found.");
                let failure_payload = WsFailurePayload::new(2210, "Member ID not found.");
                socket.emit("type:failure", &failure_payload).ok();
            }
        }
    }

    fn register_type_listeners(&self, socket: SocketRef, secure: bool) {
        let member = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();

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

            if secure {
                let frequency_monitor = Arc::new(FrequencyMonitor::new(
                    debounce_duration,
                    max_process_wait,
                    max_process_stack_size,
                ));

                socket.on("type", {
                    let frequency_monitor = frequency_monitor.clone();
                    let timeout_monitor = timeout_monitor.clone();
                    let manager_clone = self.clone();
                    async move |socket: SocketRef, Data::<TypeEventPayload>(TypeEventPayload { character, rid })| {
                        let processor = async move {
                            frequency_monitor
                                .call(character, rid, move |chars: Vec<char>, rid: i32| {
                                    Self::handle_typing(manager_clone, socket, chars, rid)
                                })
                                .await;
                        };

                        timeout_monitor.call(processor).await;
                    }
                });
            } else {
                // No frequency monitor here
                socket.on("progress", {
                    let manager_clone = self.clone();
                    async move |socket: SocketRef, Data::<ProgressEventPayload>(progress)| {
                        let processor = async move {
                            Self::handle_progress(manager_clone, socket, progress).await;
                        };

                        timeout_monitor.call(processor).await;
                    }
                });
            }
        }
    }

    fn register_base_listeners(self, socket: SocketRef, spectator: bool) {
        let member = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();

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
            let member_id = member.id.clone();
            move |s: SocketRef| {
                let mc_leave = manager_clone_leave.clone();
                let socket_leave = s.clone();
                async move {
                    info!(
                        "Member {} is attempting to leave tournament {}",
                        member_id, mc_leave.inner.tournament_id
                    );
                    if !spectator {
                        mc_leave
                            .handle_participant_leave(&member_id, &socket_leave)
                            .await
                            .map_err(|e| {
                                warn!(
                                    "Error during leave handling for member {}: {}",
                                    member_id, e
                                );
                            })
                            .ok();
                    }
                    let leave_success_payload = LeaveSuccessPayload {
                        message: "Left tournament successfully".to_string(),
                    };
                    if s.emit("leave:success", &leave_success_payload).is_err() {
                        warn!("Failed to send leave:success to {}: {}", member_id, s.id);
                    }
                }
            }
        });

        socket.on_disconnect({
            let manager_clone_disconnect = self.clone();
            move |s: SocketRef| {
                let mc_disconnect = manager_clone_disconnect.clone();
                let member = s.extensions.get::<Arc<TournamentRoomMember>>().unwrap();
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
                    let member_me = s.extensions.get::<Arc<TournamentRoomMember>>().unwrap();
                    let cid_me = (*member_me).id.clone();
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
        &self,
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

    pub async fn handle_timeout(self, socket: SocketRef) {
        let member = socket
            .extensions
            .get::<Arc<TournamentRoomMember>>()
            .unwrap();
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

    pub async fn shutdown(&self) {
        let now = Utc::now();
        self.inner
            .tournament_session_state
            .lock()
            .await
            .ended_at
            .get_or_insert(now);

        info!(
            "Shutting down manager for tournament {}",
            &*self.inner.tournament_id
        );

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

        self.inner.broadcast_update_data(false).await;

        self.update_all_broadcaster.shutdown().await;

        let manager_clone = self.clone();
        let evict_task = async move {
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

        let evict_on = Utc::now() + TimeDelta::minutes(10);
        crate::scheduler::schedule_new_task(evict_task, evict_on).ok();
    }
}
