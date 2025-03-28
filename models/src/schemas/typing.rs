use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::user::UserSession;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypingSession {
    pub client_id: String,
    pub user_id: Option<i32>,
    pub user_name: Option<String>,
    pub tournament_id: String,
    pub started_at: Option<DateTime<Utc>>, // Specific to the session
    pub ended_at: Option<DateTime<Utc>>,   // Specific to the session
    pub current_position: usize,
    pub correct_position: usize,
    pub total_keystrokes: i32,
    pub current_accuracy: f32,
    pub current_speed: f32, // WPM
}

impl TypingSession {
    pub fn new(session: &UserSession, tournament_id: String) -> Self {
        Self {
            client_id: session.client_id.clone(),
            user_id: session.user.as_ref().map(|u| u.id),
            user_name: session.user.as_ref().map(|u| u.username.clone()),
            tournament_id,
            started_at: Some(Utc::now()),
            ended_at: None,
            current_position: 0,
            correct_position: 0,
            total_keystrokes: 0,
            current_accuracy: 100.0,
            current_speed: 0.0,
        }
    }

    pub fn update(
        &mut self,
        current_position: usize,
        correct_position: usize,
        total_keystrokes: i32,
        current_accuracy: f32,
        current_speed: f32,
    ) {
        self.current_position = current_position;
        self.correct_position = correct_position;
        self.total_keystrokes = total_keystrokes;
        self.current_accuracy = current_accuracy;
        self.current_speed = current_speed;
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TypingSessionSchema {
    pub user_id: String,
    pub user_name: Option<String>,
    pub tournament_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub current_position: usize,
    pub correct_position: usize,
    pub total_keystrokes: i32,
    pub current_accuracy: i32,
    pub current_speed: i32,
}

impl From<TypingSession> for TypingSessionSchema {
    fn from(t: TypingSession) -> Self {
        Self {
            user_id: t.client_id,
            user_name: t.user_name,
            tournament_id: t.tournament_id,
            started_at: t.started_at,
            ended_at: t.ended_at,
            current_position: t.current_position,
            correct_position: t.correct_position,
            total_keystrokes: t.total_keystrokes,
            current_accuracy: t.current_accuracy as _,
            current_speed: t.current_speed as _,
        }
    }
}
