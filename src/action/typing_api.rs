use crate::{
    cache::{
        redis_delete_typing_session, redis_get_tournament, redis_get_typing_session,
        redis_set_typing_session,
    },
    TypingSession, JOIN_DEADLINE,
};

use super::UserSession;
use app::persistence::tournaments::get_tournament;
use chrono::{TimeDelta, Utc};
use sea_orm::DatabaseConnection;
use socketioxide::extract::SocketRef;
use tracing::info;

pub async fn handle_join(
    tournament_id: String,
    user: UserSession,
    socket: SocketRef,
    conn: DatabaseConnection,
) {
    info!("Received join event: {:?}", tournament_id);
    let user_session = redis_get_typing_session(&user.client_id).await;
    if let Some(session) = user_session {
        if session.tournament_id == tournament_id {
            socket.emit("join-back", &"Already joined").ok();
        } else {
            // Leave the current session and join the new one if it's valid and still open
            let tournament = get_tournament(&conn, tournament_id).await.unwrap();
            if let Some(tournament) = tournament {
                if tournament.scheduled_for - Utc::now() >= TimeDelta::seconds(JOIN_DEADLINE) {
                    // Allow joining the tournament
                    redis_delete_typing_session(&session.tournament_id, &user.client_id).await;
                    let new_session = TypingSession::new(
                        user.client_id.clone(),
                        user.user_id.clone(),
                        tournament.id.clone(),
                    );
                    redis_set_typing_session(&user.client_id, new_session).await;
                    socket.emit("join-back", &"Joined").ok();
                } else {
                    socket
                        .emit("join-back", &"Tournament no longer accepting participants")
                        .ok();
                }
            } else {
                socket.emit("join-back", &"Tournament not found").ok();
            }
        }
    } else {
        socket.emit("join-back", &"No session found").ok();
    }
}

pub async fn handle_leave(tournament_id: String, user: UserSession, socket: SocketRef) {
    info!("Received leave event: {:?}", tournament_id);
    let user_session = redis_get_typing_session(&user.client_id).await;
    if let Some(session) = user_session {
        if session.tournament_id == tournament_id {
            redis_delete_typing_session(&session.tournament_id, &user.client_id).await;
            socket.emit("leave-back", &"Left").ok();
        } else {
            socket.emit("leave-back", &"Not in this tournament").ok();
        }
    } else {
        socket.emit("leave-back", &"No session found").ok();
    }
}

pub async fn handle_typing(
    user: UserSession,
    socket: SocketRef,
    input_char: char, // The character input from the user
) {
    let mut typing_session = redis_get_typing_session(&user.client_id).await;
    let tournament = redis_get_tournament(&tournament_id).await;
    let challenge_text = match tournament {
        Some(tournament) => tournament.text,
        None => {
            socket.emit("typing-error", &"Invalid tournament").ok();
            return;
        }
    };
    info!("Received typing event for tournament: {:?}", tournament_id);

    // Retrieve the user's typing session
    let mut typing_session = match redis_get_typing_session(&user.client_id).await {
        Some(session) => session,
        None => {
            socket
                .emit("typing-error", &"No active typing session found")
                .ok();
            return;
        }
    };

    // Ensure the session is for the correct tournament
    if typing_session.tournament_id != tournament_id {
        socket.emit("typing-error", &"Invalid tournament").ok();
        return;
    }

    // Initialize start time if not already set
    if typing_session.started_at.is_none() {
        typing_session.started_at = Some(Utc::now());
    }

    // Process the input character
    if input_char == '\u{8}' {
        // Handle backspace
        if typing_session.current_position > typing_session.correct_position {
            typing_session.current_position -= 1;
        } else if typing_session.current_position > 0 {
            typing_session.current_position -= 1;
            typing_session.correct_position -= 1;
        }
    } else {
        // Increment total keystrokes
        typing_session.total_keystrokes += 1;

        // Check if the character is correct
        if typing_session.current_position < challenge_text.len() {
            let expected_char = challenge_text[typing_session.current_position];
            if input_char == expected_char {
                typing_session.correct_position += 1;
            }
            typing_session.current_position += 1;
        }
    }

    // Check if the user has completed the challenge
    if typing_session.correct_position >= challenge_text.len() && typing_session.ended_at.is_none()
    {
        typing_session.ended_at = Some(Utc::now());
    }

    // Calculate WPM and accuracy
    let elapsed_time = typing_session
        .started_at
        .map(|start| Utc::now().signed_duration_since(start).num_seconds())
        .unwrap_or(0) as f32
        / 60.0; // Convert to minutes

    typing_session.current_speed = if elapsed_time > 0.0 {
        typing_session.correct_position as f32 / 5.0 / elapsed_time
    } else {
        0.0
    };

    typing_session.current_accuracy = if typing_session.total_keystrokes > 0 {
        (typing_session.correct_position as f32 / typing_session.total_keystrokes as f32) * 100.0
    } else {
        100.0
    };

    // Save the updated session
    redis_set_typing_session(&user.client_id, typing_session.clone()).await;

    // Emit the updated session to the client
    socket.emit("typing-update", &typing_session).ok();

    // If the user has completed the challenge, notify them
    if typing_session.correct_position >= challenge_text.len() {
        socket.emit("typing-complete", &"Challenge completed!").ok();
    }
}
