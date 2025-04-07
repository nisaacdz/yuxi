use crate::{
    cache::{
        cache_delete_typing_session, cache_get_tournament, cache_get_tournament_participants,
        cache_get_typing_session, cache_set_tournament, cache_set_typing_session,
        cache_update_tournament,
    },
    scheduler::schedule_new_task,
};

const JOIN_DEADLINE_SECONDS: i64 = 15;

use super::ClientSchema;
use app::persistence::{text::get_or_generate_text, tournaments::get_tournament};
use chrono::{TimeDelta, Utc};
use models::schemas::{
    tournament::{TournamentSchema, TournamentSession},
    typing::{TournamentUpdateSchema, TypingSessionSchema},
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use socketioxide::extract::SocketRef;
use tracing::info;

#[derive(Serialize, Debug)]
struct ApiResponse<T: Serialize> {
    success: bool,
    message: String,
    data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(message: &str, data: Option<T>) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data,
        }
    }

    fn error(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }
}

pub async fn try_join_tournament(
    conn: DatabaseConnection,
    tournament_id: &String,
    user: &ClientSchema,
) -> Result<TournamentSession, String> {
    let tournament = get_tournament(&conn, tournament_id.clone()).await.unwrap();
    if let Some(tournament) = tournament {
        if tournament.scheduled_for - Utc::now() >= TimeDelta::seconds(JOIN_DEADLINE_SECONDS) {
            // Allow joining the tournament
            let new_session = TypingSessionSchema::new(user.clone(), tournament.id.clone());
            let new_tournament = schedule_tournament(conn, tournament).await?;
            cache_set_typing_session(new_session).await;
            Ok(new_tournament)
        } else {
            Err("Tournament no longer accepting participants".to_string())
        }
    } else {
        Err("Tournament not found".to_string())
    }
}

pub async fn handle_join(tournament_id: String, socket: SocketRef, conn: DatabaseConnection) {
    let user = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
    info!("Received join event: {:?}", tournament_id);

    let response: ApiResponse<TournamentSession> = match cache_get_typing_session(&user.id).await {
        Some(session) if session.tournament_id == tournament_id => {
            ApiResponse::error("Already joined this tournament")
        }
        Some(existing_session) => match try_join_tournament(conn, &tournament_id, user).await {
            Ok(t) => {
                cache_delete_typing_session(&existing_session.tournament_id, &user.id).await;
                ApiResponse::success("Switched tournaments", Some(t))
            }
            Err(e) => ApiResponse::error(&e),
        },
        None => match try_join_tournament(conn, &tournament_id, user).await {
            Ok(t) => ApiResponse::success("Joined tournament", Some(t)),
            Err(e) => ApiResponse::error(&e),
        },
    };

    socket.emit("join:response", &response).ok();

    if response.success {
        socket.join(tournament_id.clone());
        socket
            .to(tournament_id.clone())
            .emit("user:joined", user)
            .await
            .ok();

        let participants = cache_get_tournament_participants(&tournament_id).await;
        let tournament_update = TournamentUpdateSchema::new(response.data.unwrap(), participants);
        socket
            .to(tournament_id.clone())
            .emit("tournament:update", &tournament_update)
            .await
            .ok();
    }
}

pub async fn handle_leave(tournament_id: String, socket: SocketRef) {
    let user = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
    info!("Received leave event: {:?}", tournament_id);

    let response = match cache_get_typing_session(&user.id).await {
        Some(session) if session.tournament_id == tournament_id => {
            cache_delete_typing_session(&tournament_id, &user.id).await;
            cache_update_tournament(&tournament_id, |t| t.current -= 1).await;
            ApiResponse::success("Left tournament", None::<()>)
        }
        Some(_) => ApiResponse::error("Not in this tournament"),
        None => ApiResponse::error("No active session"),
    };

    socket.emit("leave:response", &response).ok();

    if response.success {
        socket.to(tournament_id).emit("user:left", user).await.ok();
    }

    socket.disconnect().ok();
}

pub async fn handle_timeout(client: &ClientSchema, socket: SocketRef) {
    let tournament = if let Some(ts) = cache_get_typing_session(&client.id).await {
        cache_get_tournament(&ts.tournament_id).await
    } else {
        None
    };
    if let Some(tournament) = tournament {
        handle_leave(tournament.id, socket).await
    }
}

pub async fn handle_typing(socket: SocketRef, typed_chars: Vec<char>) {
    let user = socket.req_parts().extensions.get::<ClientSchema>().unwrap();
    let now = Utc::now();
    info!("Received typing event: {:?}", typed_chars);

    let mut typing_session = match cache_get_typing_session(&user.id).await {
        Some(session) => session,
        None => {
            socket
                .emit(
                    "typing:error",
                    &ApiResponse::<()>::error("No active session"),
                )
                .ok();
            return;
        }
    };

    let tournament = match cache_get_tournament(&typing_session.tournament_id).await {
        Some(tournament) => tournament,
        None => {
            socket
                .emit(
                    "typing:error",
                    &ApiResponse::<()>::error("Tournament not found"),
                )
                .ok();
            return;
        }
    };

    let challenge_text = if let Some(text) = &tournament.text {
        text.as_bytes()
    } else {
        return;
    };

    // Initialize start time if not already set
    if typing_session.started_at.is_none() {
        typing_session.started_at = Some(now);
    }

    // Process all characters in the input sequence
    for current_char in typed_chars {
        // Break early if challenge completed
        if typing_session.correct_position >= challenge_text.len() {
            break;
        }

        if current_char == '\u{8}' {
            // Backspace character
            // Handle backspace logic
            if typing_session.current_position > typing_session.correct_position {
                typing_session.current_position -= 1;
            } else if typing_session.current_position == typing_session.correct_position {
                if typing_session.current_position > 0 {
                    // Only move correct position back if previous character wasn't a space
                    if challenge_text[typing_session.current_position - 1] != b' ' {
                        typing_session.correct_position -= 1;
                    }
                    typing_session.current_position -= 1;
                }
            }
        } else {
            // Regular character processing
            typing_session.total_keystrokes += 1;

            // Only process if within challenge bounds
            if typing_session.current_position < challenge_text.len() {
                if typing_session.current_position == typing_session.correct_position
                    && current_char as u8 == challenge_text[typing_session.current_position]
                {
                    typing_session.correct_position += 1;
                }
                typing_session.current_position += 1;
            }
        }
    }

    // Check for challenge completion
    if typing_session.correct_position >= challenge_text.len() && typing_session.ended_at.is_none()
    {
        typing_session.ended_at = Some(now);
    }

    // Calculate metrics
    let elapsed_time = typing_session
        .started_at
        .unwrap()
        .signed_duration_since(now);
    let minutes_elapsed = (-elapsed_time.num_seconds() as f32) / 60.0; // Convert to positive minutes

    typing_session.current_speed = if minutes_elapsed > 0.0 {
        (typing_session.correct_position as f32 / 5.0 / minutes_elapsed).round()
    } else {
        0.0
    };

    typing_session.current_accuracy = if typing_session.total_keystrokes > 0 {
        ((typing_session.correct_position as f32 / typing_session.total_keystrokes as f32) * 100.0)
            .round()
    } else {
        100.0
    };

    // Save updated session
    cache_set_typing_session(typing_session.clone()).await;

    // Broadcast update
    let response = ApiResponse::success(
        "Progress updated",
        Some(TypingSessionSchema::from(typing_session)),
    );
    socket
        .to(tournament.id)
        .emit("typing:update", &response)
        .await
        .ok();
}

pub async fn schedule_tournament(
    conn: DatabaseConnection,
    tournament: TournamentSchema,
) -> Result<TournamentSession, String> {
    if let Some(cache_tournament) = cache_get_tournament(&tournament.id).await {
        return Ok(cache_tournament);
    }

    {
        let tournament = TournamentSchema::from(tournament.clone());
        let tournament_id = tournament.id.clone();
        let conn = conn.clone();
        let task = async move {
            let text = match get_or_generate_text(&conn, &tournament.id).await {
                Ok(text) => text,
                Err(err) => {
                    tracing::error!("Error generating typing text: {}", err);
                    return;
                }
            };
            cache_update_tournament(&tournament.id, |t| {
                t.text = Some(text);
                t.started_at = Some(Utc::now());
            })
            .await;
        };
        schedule_new_task(tournament_id, task, tournament.scheduled_for)
            .await
            .ok();
    }

    let text = None;

    let new_cached_tournament =
        TournamentSession::new(tournament.id.clone(), tournament.scheduled_for, text);

    cache_set_tournament(&tournament.id, new_cached_tournament).await;

    cache_get_tournament(&tournament.id)
        .await
        .ok_or("An error occurred when scheduling the tournament".into())
}
