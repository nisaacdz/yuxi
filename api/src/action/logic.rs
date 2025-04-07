//! Contains business logic related to tournament state and scheduling.

use crate::{
    cache::{
        cache_get_tournament, cache_get_tournament_participants, cache_set_tournament,
        cache_set_typing_session, cache_update_tournament,
    },
    scheduler::schedule_new_task,
};
use app::persistence::{text::get_or_generate_text, tournaments::get_tournament};
use chrono::{TimeDelta, Utc};
use models::schemas::{
    tournament::{TournamentSchema, TournamentSession},
    typing::{TournamentUpdateSchema, TypingSessionSchema},
    user::ClientSchema,
};
use sea_orm::DatabaseConnection;
use socketioxide::extract::SocketRef;
use tracing::error;

/// Defines the time window before a tournament starts during which joining is still allowed.
const JOIN_DEADLINE_SECONDS: i64 = 15;

/// Attempts to register a user for a specific tournament.
///
/// Checks if the tournament exists and if the join deadline has passed.
/// If joinable, creates a new typing session for the user and schedules the tournament
/// if it hasn't been scheduled already.
///
/// # Arguments
///
/// * `conn` - A database connection.
/// * `tournament_id` - The ID of the tournament to join.
/// * `user` - The client information of the user joining.
///
/// # Returns
///
/// * `Ok(TournamentSession)` - The session details of the tournament joined.
/// * `Err(String)` - An error message indicating why joining failed (e.g., not found, deadline passed).
pub async fn try_join_tournament(
    conn: &DatabaseConnection,
    tournament_id: &String,
    user: &ClientSchema,
    socket: &SocketRef,
) -> Result<TournamentSession, String> {
    // Fetch tournament details from the database
    let tournament_result =
        get_tournament(conn, tournament_id.clone())
            .await
            .map_err(|db_err| {
                error!(
                    "Database error fetching tournament {}: {}",
                    tournament_id, db_err
                );
                "Failed to retrieve tournament details.".to_string()
            })?;

    if let Some(tournament) = tournament_result {
        // Check if the joining deadline has passed
        let now = Utc::now();
        let join_deadline = tournament.scheduled_for - TimeDelta::seconds(JOIN_DEADLINE_SECONDS);

        if now < join_deadline {
            // Allow joining the tournament
            let new_session = TypingSessionSchema::new(user.clone(), tournament.id.clone());

            // Schedule the tournament (ensures text generation task is set up)
            // This is idempotent due to cache check inside schedule_tournament.
            let scheduled_tournament = schedule_tournament(conn, tournament, socket).await?; // Propagate scheduling errors

            // Cache the user's new typing session
            cache_set_typing_session(new_session).await; // Assuming cache operations don't return critical errors

            let participants = cache_get_tournament_participants(&scheduled_tournament.id).await;
            let tournament_update =
                TournamentUpdateSchema::new(scheduled_tournament.clone(), participants);

            socket
                .to(scheduled_tournament.id.clone())
                .emit("tournament:update", &tournament_update)
                .await
                .ok();
            Ok(scheduled_tournament)
        } else {
            Err("Tournament join deadline has passed.".to_string())
        }
    } else {
        Err("Tournament not found.".to_string())
    }
}

/// Schedules the background task for a tournament (e.g., text generation) if not already scheduled.
///
/// Checks the cache first. If the tournament session isn't cached, it creates a cache entry
/// and schedules an async task to run at the tournament's start time. This task
/// generates the typing text and updates the cached tournament state.
///
/// # Arguments
///
/// * `conn` - A database connection (cloned for the async task).
/// * `tournament` - The database schema of the tournament to schedule.
///
/// # Returns
///
/// * `Ok(TournamentSession)` - The newly created or existing cached tournament session.
/// * `Err(String)` - An error message if scheduling fails.
pub async fn schedule_tournament(
    conn: &DatabaseConnection,
    tournament: TournamentSchema,
    socket: &SocketRef,
) -> Result<TournamentSession, String> {
    // Check cache first to ensure idempotency
    if let Some(cached_tournament) = cache_get_tournament(&tournament.id).await {
        return Ok(cached_tournament);
    }

    // --- Schedule the text generation task ---
    {
        // Clone necessary variables for the async task
        let tournament_for_task = tournament.clone();
        let tournament_id_for_task = tournament.id.clone();
        let conn_for_task = conn.clone();
        let socket = socket.clone();
        let task = async move {
            match get_or_generate_text(&conn_for_task, &tournament_id_for_task).await {
                Ok(text) => {
                    // Update the cached tournament with the generated text and start time
                    cache_update_tournament(&tournament_id_for_task, |t| {
                        t.text = Some(text);
                        t.started_at = Some(Utc::now());
                    })
                    .await;
                    let started_tournament =
                        match cache_get_tournament(&tournament_id_for_task).await {
                            Some(v) => v,
                            None => {
                                return error!("Tournament session not found");
                            }
                        };
                    socket
                        .to(tournament_id_for_task)
                        .emit("tournament:start", &started_tournament)
                        .await
                        .ok();
                }
                Err(err) => {
                    error!(
                        "Error generating/fetching typing text for tournament {}: {}",
                        tournament_id_for_task, err
                    );
                    // TODO: How to handle this failure? Maybe update cache with an error state?
                    // For now, it just logs. The tournament might proceed without text.
                }
            }
        };

        // Schedule the task to run at the designated time
        if let Err(schedule_err) = schedule_new_task(
            tournament.id.clone(),             // Task ID
            task,                              // Task future
            tournament_for_task.scheduled_for, // Execution time
        )
        .await
        {
            error!(
                "Failed to schedule task for tournament {}: {}",
                tournament.id, schedule_err
            );
            // Decide if this is a critical failure. For now, we continue and cache without the task guaranteed.
            // return Err(format!("Failed to schedule tournament task: {}", schedule_err));
        }
    }
    // --- Task scheduled (or failed silently/logged) ---

    // Create the initial cached representation of the tournament session
    let new_cached_tournament =
        TournamentSession::new(tournament.id.clone(), tournament.scheduled_for, None); // Text is initially None

    // Store the initial session in the cache
    cache_set_tournament(&tournament.id, new_cached_tournament).await; // Assume cache set succeeds

    // Retrieve from cache to ensure consistency and return the cached value
    cache_get_tournament(&tournament.id).await.ok_or_else(|| {
        error!(
            "Failed to retrieve tournament {} from cache immediately after setting it.",
            tournament.id
        );
        "An internal error occurred when scheduling the tournament.".to_string()
    })
}
