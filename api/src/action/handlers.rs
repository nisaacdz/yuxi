use super::state::ApiResponse;
use crate::cache::Cache;

use chrono::{DateTime, Utc};
use models::schemas::{typing::TypingSessionSchema, user::ClientSchema};

use std::sync::Arc;

use socketioxide::{SocketIo, extract::SocketRef};
use tracing::{info, warn};

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
pub async fn handle_typing(
    io: SocketIo,
    socket: SocketRef,
    typed_chars: Vec<char>,
    cache: Cache<TypingSessionSchema>,
    typing_text: Arc<String>,
) {
    let client = socket.req_parts().extensions.get::<ClientSchema>().unwrap();

    if typed_chars.is_empty() {
        warn!(client_id = %client.id, "Received empty typing event. Ignoring.");
        return;
    }

    let typing_session = match cache.get_data(&client.id) {
        Some(session) => session,
        None => {
            warn!(client_id = %client.id, "Typing event received, but no active session found.");
            let _ = socket.emit(
                "typing:error",
                &ApiResponse::<()>::error("No active typing session found."),
            );
            return;
        }
    };

    let challenge_text_bytes = typing_text.as_bytes();

    // --- Process Input and Update State ---
    let now = Utc::now();
    let updated_session =
        process_typing_input(typing_session, typed_chars, challenge_text_bytes, now);

    cache.set_data(&updated_session.client.id, updated_session.clone());

    let response = ApiResponse::success("Progress updated", Some(updated_session));

    let tournament_id = socket.ns().trim_start_matches("/tournament/").to_string();

    if let Err(e) = io
        .to(tournament_id.to_owned())
        .emit("typing:update", &response)
        .await
    {
        warn!(client_id = %client.id, tournament_id = %tournament_id, error = %e, "Failed to broadcast typing:update");
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
