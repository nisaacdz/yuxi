use super::{logic::try_join_tournament, state::ApiResponse};
use crate::cache::{
    cache_delete_typing_session, cache_get_tournament, cache_get_tournament_participants,
    cache_get_typing_session, cache_set_typing_session, cache_update_tournament,
};

use chrono::{DateTime, Utc};
use models::schemas::{
    tournament::TournamentSession,
    typing::{TournamentUpdateSchema, TypingSessionSchema},
    user::ClientSchema,
};
use sea_orm::DatabaseConnection;
use socketioxide::{SocketIo, extract::SocketRef};
use tracing::{error, info, warn};

/// Handles a client's request to join a tournament.
///
/// Validates the request, checks current session state, attempts to join via `try_join_tournament`,
/// updates cache, joins the socket room, and broadcasts updates to participants.
///
/// # Arguments
///
/// * `tournament_id` - The ID of the tournament the user wants to join.
/// * `socket` - The user's socket connection reference.
/// * `conn` - A database connection.
pub async fn handle_join(
    tournament_id: String,
    io: SocketIo,
    socket: SocketRef,
    conn: DatabaseConnection,
) {
    let user = match socket.req_parts().extensions.get::<ClientSchema>() {
        Some(client) => client.clone(),
        None => {
            error!(
                "ClientSchema not found in socket extensions for ID: {}",
                socket.id
            );
            let _ = socket.emit(
                "join:response",
                &ApiResponse::<()>::error("Internal server error"),
            );
            return;
        }
    };
    info!(user_id = %user.id, %tournament_id, "Received join request");

    let current_session = cache_get_typing_session(&user.id).await;
    let join_result: Result<TournamentSession, String>;

    match &current_session {
        // Case 1: User is already in the target tournament
        Some(session) if session.tournament_id == tournament_id => {
            join_result = {
                match cache_get_tournament(&tournament_id)
                    .await
                    .map(|t| t.clone())
                {
                    Some(tournament) => Ok(tournament),
                    None => Err("Failed to retrieve tournament info".to_owned()),
                }
            };
        }

        Some(existing_session) => {
            info!(user_id = %user.id, old_tournament_id = %existing_session.tournament_id, new_tournament_id = %tournament_id, "User switching tournaments");
            match try_join_tournament(&conn, &tournament_id, &user, &socket).await {
                Ok(new_tournament_session) => {
                    socket.leave(existing_session.tournament_id.clone());
                    cache_delete_typing_session(&existing_session.tournament_id, &user.id).await;
                    join_result = Ok(new_tournament_session);
                }
                Err(e) => join_result = Err(e),
            }
        }
        // Case 3: User is not in any tournament, try joining
        None => {
            join_result = try_join_tournament(&conn, &tournament_id, &user, &socket).await;
        }
    }

    // Construct and send the response
    let response = match join_result {
        Ok(session)
            if current_session.is_some()
                && current_session.as_ref().unwrap().tournament_id != tournament_id =>
        {
            ApiResponse::success("Switched tournaments successfully", Some(session))
        }
        Ok(session) => ApiResponse::success("Joined tournament successfully", Some(session)),
        Err(e) => ApiResponse::error(&e),
    };

    socket.emit("join:response", &response).ok();

    if response.is_success() {
        if let Some(joined_tournament_session) = response.into_data() {
            // Safely get data
            let room_id = joined_tournament_session.id.clone();
            // Join the socket room for the new tournament
            socket.join(room_id.clone());

            io.to(room_id.clone()).emit("user:joined", &user).await.ok();

            let participants = cache_get_tournament_participants(&room_id).await;
            let tournament_update =
                TournamentUpdateSchema::new(joined_tournament_session, participants);

            let v = io
                .to(room_id.clone())
                .emit("tournament:update", &tournament_update)
                .await;

            if let Err(e) = v {
                warn!(user_id = %user.id, tournament_id = %room_id, error = %e, "Failed to broadcast tournament:update");
            } else {
                info!("updated tournament successfully");
            }
        } else {
            // not supposed to happen? think me carefully but later
            error!(user_id = %user.id, %tournament_id, "Join response was successful but contained no data!");
        }
    }
}

/// Handles a client's request to leave a tournament.
///
/// Removes the user's session from the cache, updates the participant count,
/// leaves the socket room, and notifies other participants.
///
/// # Arguments
///
/// * `tournament_id` - The ID of the tournament the user wants to leave.
/// * `socket` - The user's socket connection reference.
pub async fn handle_leave(io: SocketIo, socket: SocketRef, tournament_id: String) {
    let user = match socket.req_parts().extensions.get::<ClientSchema>() {
        Some(client) => client.clone(),
        None => {
            error!(
                "ClientSchema not found in socket extensions for ID: {}",
                socket.id
            );
            return;
        }
    };
    info!(user_id = %user.id, %tournament_id, "Received leave request");

    let response: ApiResponse<()> = match cache_get_typing_session(&user.id).await {
        Some(session) if session.tournament_id == tournament_id => {
            // User is in the specified tournament, proceed with leaving
            cache_delete_typing_session(&tournament_id, &user.id).await;
            // Decrement participant count (best effort, ignore if tournament not found in cache)
            let _ = cache_update_tournament(&tournament_id, |t| {
                if t.current > 0 {
                    t.current -= 1
                } // Prevent underflow
            })
            .await;
            ApiResponse::success("Left tournament successfully", None)
        }
        Some(session) => {
            // User is in a *different* tournament
            warn!(user_id = %user.id, expected_tournament_id = %tournament_id, actual_tournament_id = %session.tournament_id, "User tried to leave wrong tournament");
            ApiResponse::error("You are not in this tournament.")
        }
        None => {
            // User is not in any active session
            warn!(user_id = %user.id, %tournament_id, "User tried to leave tournament but has no active session");
            ApiResponse::error("You do not have an active typing session.")
        }
    };

    if let Err(e) = socket.emit("leave:response", &response) {
        warn!(user_id = %user.id, error = %e, "Failed to send leave:response");
    }

    if response.is_success() {
        // Leave the socket room
        socket.leave(tournament_id.clone());

        // Notify others in the room
        if let Err(e) = io
            .to(tournament_id.clone())
            .emit("user:left", &user) // Send the user who left
            .await
        {
            warn!(user_id = %user.id, %tournament_id, error = %e, "Failed to broadcast user:left");
        }

        // TODO: Fetch and broadcast updated TournamentUpdateSchema?
        // Similar to handle_join, maybe broadcast an update after leave
    }

    // Optionally disconnect the socket after leaving? Your original code did this.
    // if response.is_success() { // Only disconnect if leave was successful?
    // if let Err(e) = socket.disconnect() {
    //     error!(user_id = %user.id, socket_id = %socket.id, error = %e, "Failed to disconnect socket after leaving tournament");
    // }
    // }
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
    info!(user_id = %client.id, "Handling inactivity timeout");
    match cache_get_typing_session(&client.id).await {
        Some(_ts) => {
            // Found an active session, proceed to leave the associated tournament
            //handle_leave(ts.tournament_id, socket).await;
        }
        None => {
            // User had no active session when timeout triggered, nothing to leave.
            info!(user_id = %client.id, "Timeout triggered, but no active session found. No action needed.");
            // Optionally disconnect here if timeout implies disconnection regardless of session state
            // if let Err(e) = socket.disconnect() {
            //     error!(user_id = %client.id, socket_id = %socket.id, error = %e, "Failed to disconnect socket on timeout with no session");
            // }
        }
    }
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
pub async fn handle_typing(io: SocketIo, socket: SocketRef, typed_chars: Vec<char>) {
    let user = match socket.req_parts().extensions.get::<ClientSchema>() {
        Some(client) => client.clone(),
        None => {
            error!(
                "ClientSchema not found in socket extensions for ID: {}",
                socket.id
            );
            return;
        }
    };

    if typed_chars.is_empty() {
        warn!(user_id = %user.id, "Received empty typing event. Ignoring.");
        return;
    }

    // info!(user_id = %user.id, chars = ?typed_chars, "Received typing event"); // Potentially noisy log

    // --- Get Session and Tournament State ---
    let typing_session = match cache_get_typing_session(&user.id).await {
        Some(session) => session,
        None => {
            warn!(user_id = %user.id, "Typing event received, but no active session found.");
            let _ = socket.emit(
                "typing:error",
                &ApiResponse::<()>::error("No active typing session found."),
            );
            return;
        }
    };

    let tournament = match cache_get_tournament(&typing_session.tournament_id).await {
        Some(t) => t,
        None => {
            error!(user_id = %user.id, tournament_id = %typing_session.tournament_id, "Active session found, but corresponding tournament not in cache!");
            let _ = socket.emit(
                "typing:error",
                &ApiResponse::<()>::error("Tournament data not found. Please rejoin."),
            );
            // Consider cleaning up the orphaned session?
            // cache_delete_typing_session(&typing_session.tournament_id, &user.id).await;
            return;
        }
    };

    let challenge_text_bytes = match &tournament.text {
        Some(text) if !text.is_empty() => text.as_bytes(),
        _ => {
            warn!(user_id = %user.id, tournament_id = %typing_session.tournament_id, "Typing event received, but tournament text is missing or empty.");
            let _ = socket.emit(
                "typing:error",
                &ApiResponse::<()>::error("Tournament text is not available yet."),
            );
            return;
        }
    };

    // Check if tournament has actually started (started_at is set)
    if tournament.started_at.is_none() {
        warn!(user_id = %user.id, tournament_id = %typing_session.tournament_id, "Typing event received before tournament start time.");
        let _ = socket.emit(
            "typing:error",
            &ApiResponse::<()>::error("Tournament has not started yet."),
        );
        return;
    }

    // --- Process Input and Update State ---
    let now = Utc::now();
    let updated_session =
        process_typing_input(typing_session, typed_chars, challenge_text_bytes, now);

    // --- Save and Broadcast ---
    cache_set_typing_session(updated_session.clone()).await;

    let response = ApiResponse::success("Progress updated", Some(updated_session)); // Send the whole updated session

    if let Err(e) = io
        .to(tournament.id.clone()) // Use tournament ID from fetched data
        .emit("typing:update", &response)
        .await
    {
        warn!(user_id = %user.id, tournament_id = %tournament.id, error = %e, "Failed to broadcast typing:update");
    }
}

/// Processes typed characters against the challenge text and updates session metrics.
///
/// This is a pure function, taking the current state and input, and returning the new state.
/// It handles character matching, backspace logic, and metric calculations (WPM, accuracy).
///
/// # Arguments
/// * `session` - The current state of the user's typing session.
/// * `typed_chars` - The sequence of characters typed.
/// * `challenge_text` - The target text as bytes.
/// * `now` - The current timestamp.
///
/// # Returns
/// The updated `TypingSessionSchema`.
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
            info!(user_id = %session.client.id, tournament_id = %session.tournament_id, "User finished typing challenge");
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
