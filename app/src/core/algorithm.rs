use models::schemas::typing::TypingSessionSchema;
use tracing::{info, warn};

use crate::core::dtos::*;

pub trait TypingAlgorithm {
    fn handle_type(
        &self,
        session: &mut TypingSessionSchema,
        input: &[char],
        original: &[u8],
    ) -> Result<PartialParticipantData, WsFailurePayload>;

    fn handle_progress(
        &self,
        session: &mut TypingSessionSchema,
        progress: ProgressEventPayload,
        original: &[u8],
    ) -> Result<PartialParticipantData, WsFailurePayload>;
}

#[derive(Clone, Copy)]
pub struct ZeroProceed;

impl TypingAlgorithm for ZeroProceed {
    fn handle_type(
        &self,
        session: &mut TypingSessionSchema,
        input: &[char],
        original: &[u8],
    ) -> Result<PartialParticipantData, WsFailurePayload> {
        let now = chrono::Utc::now();
        if session.started_at.is_none() {
            session.started_at = Some(now);
        }

        let text_len = input.len();

        for &current_char in input {
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
                    if original[session.current_position - 1] != b' ' {
                        session.correct_position -= 1;
                        session.current_position -= 1;
                    }
                }
                // If current_position is 0, backspace does nothing.
                // No change to total_keystrokes for backspace.
            } else {
                session.total_keystrokes += 1;

                if session.current_position < text_len {
                    let expected_char = original[session.current_position];
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

            session.current_speed =
                (session.correct_position as f32 / 5.0 / minutes_elapsed).round();

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

        Ok(PartialParticipantData {
            current_position: Some(session.current_position),
            correct_position: Some(session.correct_position),
            total_keystrokes: Some(session.total_keystrokes),
            current_speed: Some(session.current_speed),
            current_accuracy: Some(session.current_accuracy),
            started_at: session.started_at,
            ended_at: session.ended_at,
        })
    }

    fn handle_progress(
        &self,
        session: &mut TypingSessionSchema,
        progress: ProgressEventPayload,
        original: &[u8],
    ) -> Result<PartialParticipantData, WsFailurePayload> {
        let now = chrono::Utc::now();
        let text_len = original.len();

        let ProgressEventPayload {
            correct_position,
            current_position,
            total_keystrokes,
            rid: _,
        } = progress;

        if current_position > text_len
            || correct_position > text_len
            || correct_position > current_position
        {
            return Err(WsFailurePayload::new(2212, "Invalid progress data."));
        }

        if session.ended_at.is_some() {
            return Err(WsFailurePayload::new(2211, "Your session has ended."));
        }

        if session.started_at.is_none() {
            session.started_at = Some(now);
        }

        session.current_position = current_position;
        session.correct_position = correct_position;
        session.total_keystrokes = total_keystrokes;

        if let Some(started_at) = session.started_at {
            let duration = now.signed_duration_since(started_at);
            let minutes_elapsed = (duration.num_milliseconds() as f32 / 60000.0).max(0.0001);

            session.current_speed =
                (session.correct_position as f32 / 5.0 / minutes_elapsed).round();
            session.current_accuracy = if session.total_keystrokes > 0 {
                ((session.correct_position as f32 / session.total_keystrokes as f32) * 100.0)
                    .round()
                    .clamp(0.0, 100.0)
            } else {
                100.0
            };
        }

        if session.correct_position == text_len && session.ended_at.is_none() {
            session.ended_at = Some(now);
            info!(
                member_id = %session.member.id,
                tournament_id = %session.tournament_id,
                "User finished typing challenge via progress update"
            );
        }

        Ok(PartialParticipantData {
            current_position: Some(session.current_position),
            correct_position: Some(session.correct_position),
            total_keystrokes: Some(session.total_keystrokes),
            current_speed: Some(session.current_speed),
            current_accuracy: Some(session.current_accuracy),
            started_at: session.started_at,
            ended_at: session.ended_at,
        })
    }
}
