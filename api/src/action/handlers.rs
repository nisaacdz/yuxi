//! Handles WebSocket events related to typing tournaments.

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
use socketioxide::extract::SocketRef;
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
pub async fn handle_join(tournament_id: String, socket: SocketRef, conn: DatabaseConnection) {
    // Retrieve user data attached by middleware
    let user = match socket.req_parts().extensions.get::<ClientSchema>() {
        Some(client) => client.clone(), // Clone to own the data
        None => {
            error!(
                "ClientSchema not found in socket extensions for ID: {}",
                socket.id
            );
            // Optionally send an error response if desired, though this indicates a server setup issue.
            // let _ = socket.emit("join:response", &ApiResponse::<()>::error("Internal server error: User context missing."));
            return; // Cannot proceed without user context
        }
    };
    info!(user_id = %user.id, %tournament_id, "Received join request");

    let current_session = cache_get_typing_session(&user.id).await;
    let join_result: Result<TournamentSession, String>;

    match &current_session {
        // Case 1: User is already in the target tournament
        Some(session) if session.tournament_id == tournament_id => {
            join_result = Err("Already joined this tournament".to_string());
        }
        // Case 2: User is in a *different* tournament, try switching
        Some(existing_session) => {
            info!(user_id = %user.id, old_tournament_id = %existing_session.tournament_id, new_tournament_id = %tournament_id, "User switching tournaments");
            match try_join_tournament(&conn, &tournament_id, &user, &socket).await {
                Ok(new_tournament_session) => {
                    // Leave the old tournament room and delete old session
                    socket.leave(existing_session.tournament_id.clone());
                    cache_delete_typing_session(&existing_session.tournament_id, &user.id).await;
                    // TODO: Should we broadcast a "user:left" to the old room?
                    join_result = Ok(new_tournament_session);
                    // The success message will be set later
                }
                Err(e) => join_result = Err(e), // Propagate error from try_join_tournament
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

    // If join/switch was successful, update socket rooms and broadcast
    if response.is_success() {
        if let Some(joined_tournament_session) = response.into_data() {
            // Safely get data
            let room_id = joined_tournament_session.id.clone();

            // Join the socket room for the new tournament
            socket.join(room_id.clone());

            socket
                .to(room_id.clone())
                .emit("user:joined", &user)
                .await
                .ok();

            let participants = cache_get_tournament_participants(&room_id).await;
            let tournament_update =
                TournamentUpdateSchema::new(joined_tournament_session, participants);

            let v = socket
                .to(room_id.clone())
                .emit("tournament:update", &tournament_update)
                .await;

            if let Err(e) = v {
                warn!(user_id = %user.id, tournament_id = %room_id, error = %e, "Failed to broadcast tournament:update");
            } else {
                info!("updated tournament successfully");
            }
        } else {
            // This case should logically not happen if response.is_success() is true
            // and the logic above constructed it correctly.
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
pub async fn handle_leave(tournament_id: String, socket: SocketRef) {
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
        if let Err(e) = socket
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
pub async fn handle_typing(socket: SocketRef, typed_chars: Vec<char>) {
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
        return; // No characters to process
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

    if let Err(e) = socket
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
    // Initialize start time if this is the first input
    if session.started_at.is_none() {
        session.started_at = Some(now);
    }

    let text_len = challenge_text.len();

    for current_char in typed_chars {
        // Stop processing if already finished
        if session.correct_position >= text_len && session.ended_at.is_some() {
            break;
        }

        if current_char == '\u{8}' {
            // Backspace character (`\b` or unicode backspace)
            // --- Backspace Logic ---
            if session.current_position > session.correct_position {
                // If ahead of the correct position (in an error state), just move cursor back.
                session.current_position -= 1;
            } else if session.current_position == session.correct_position
                && session.current_position > 0
            {
                // If at the correct position, move both cursors back.
                // Original logic included a check to prevent backing over spaces - keeping that.
                // This prevents WPM from artificially dropping low if someone pauses and hits backspace.
                // If challenge_text[session.current_position - 1] != b' ' {
                session.correct_position -= 1; // Allow backspacing over correct chars
                // }
                session.current_position -= 1;
            }
            // If current_position is 0, backspace does nothing.
            // No change to total_keystrokes for backspace in this logic.
        } else {
            // --- Regular Character Processing ---
            session.total_keystrokes += 1;

            // Only process if cursor is still within the text bounds
            if session.current_position < text_len {
                // Check if the typed character matches the expected character *at the current position*
                let expected_char = challenge_text[session.current_position];
                if session.current_position == session.correct_position
                    && (current_char as u32) == (expected_char as u32)
                {
                    // Correct character typed at the right position
                    session.correct_position += 1;
                }
                // Always advance the current (typed) position
                session.current_position += 1;
            }
            // If current_position >= text_len, typed characters are ignored but still count towards keystrokes.
        }

        // Check for challenge completion *after* processing the character
        // Must be exactly at the end and not already finished.
        if session.correct_position == text_len && session.ended_at.is_none() {
            session.ended_at = Some(now);
            // Set current position potentially beyond text_len if they typed extra chars before finishing?
            // Or clamp it? Let's clamp it for consistency.
            session.current_position = session.correct_position;
            info!(user_id = %session.client.id, tournament_id = %session.tournament_id, "User finished typing challenge");
            break; // Stop processing further input after finishing
        }

        // Clamp current_position just in case logic allows it to exceed text_len undesirably
        // session.current_position = session.current_position.min(text_len);
    } // End character processing loop

    // --- Calculate Metrics ---
    if let Some(started_at) = session.started_at {
        // Use ended_at if available, otherwise use 'now' for in-progress calculation.
        let end_time = session.ended_at.unwrap_or(now);
        let duration = end_time.signed_duration_since(started_at);

        // Ensure duration is non-negative and non-zero for calculations. Use a small epsilon.
        let minutes_elapsed = (duration.num_milliseconds() as f32 / 60000.0).max(0.0001); // Avoid division by zero, use positive elapsed time

        // WPM: (Correct Chars / 5) / Minutes
        session.current_speed = (session.correct_position as f32 / 5.0 / minutes_elapsed).round();

        // Accuracy: Correct Chars / Total Keystrokes (excluding backspaces per this logic)
        session.current_accuracy = if session.total_keystrokes > 0 {
            ((session.correct_position as f32 / session.total_keystrokes as f32) * 100.0)
                .round()
                .clamp(0.0, 100.0) // Clamp between 0 and 100
        } else {
            100.0 // Perfect accuracy if no keystrokes yet
        };
    } else {
        // Should not happen if processing starts correctly, but defensively set to 0.
        session.current_speed = 0.0;
        session.current_accuracy = 100.0;
    }

    session // Return the updated session
}
