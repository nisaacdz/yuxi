use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schemas::user::TournamentRoomMember;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypingSessionSchema {
    pub member: TournamentRoomMember,
    pub tournament_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub current_position: usize,
    pub correct_position: usize,
    pub total_keystrokes: i32,
    pub current_accuracy: f32,
    pub current_speed: f32,
}

impl TypingSessionSchema {
    pub fn new(member: TournamentRoomMember, tournament_id: String) -> Self {
        Self {
            member,
            tournament_id,
            started_at: None,
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

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[serde(untagged)]
pub enum TournamentStatus {
    Upcoming,
    Started,
    Ended,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextOptions {
    uppercase: bool,
    lowercase: bool,
    numbers: bool,
    symbols: bool,
    meaningful: bool,
}

impl TextOptions {
    pub fn from_value(value: serde_json::Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }

    pub fn to_value(self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            uppercase: true,
            lowercase: true,
            numbers: true,
            symbols: true,
            meaningful: true,
        }
    }
}
