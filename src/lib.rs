use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "shuttle")]
pub mod shuttle;
#[cfg(not(feature = "shuttle"))]
pub mod tokio;

pub(crate) mod action;
pub(crate) mod cache;
pub(crate) mod middleware;

pub(self) const JOIN_DEADLINE: i64 = 15;

#[derive(Clone, Debug)]
pub struct UserSession {
    pub client_id: String,
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TournamentInfo {
    pub id: String,
    pub started_at: Option<DateTime<Local>>,
    pub ended_at: Option<DateTime<Local>>,
    pub text: Vec<char>,
    pub total_joined: i32,
    pub total_remaining: i32,
    pub total_completed: i32,
    pub automatized: bool,
}

impl TournamentInfo {
    pub fn new(id: String, text: Vec<char>) -> Self {
        Self {
            id,
            started_at: None,
            ended_at: None,
            text,
            total_joined: 0,
            total_remaining: 0,
            total_completed: 0,
            automatized: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypingSession {
    pub client_id: String,
    pub user_id: Option<String>,
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
    pub fn new(client_id: String, user_id: Option<String>, tournament_id: String) -> Self {
        Self {
            client_id,
            user_id,
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
            user_id: if let Some(user_id) = t.user_id {
                user_id
            } else {
                t.client_id
            },
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
