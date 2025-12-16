use chrono::{DateTime, Utc};
use models::schemas::user::TournamentRoomMember;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone)]
pub struct WsFailurePayload {
    pub code: i32,
    pub message: String,
}

impl WsFailurePayload {
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantData {
    pub member: TournamentRoomMember,
    pub current_position: usize,
    pub correct_position: usize,
    pub total_keystrokes: i32,
    pub current_speed: f32,
    pub current_accuracy: f32,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct PartialParticipantData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_keystrokes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_accuracy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartialParticipantDataForUpdate<'a> {
    pub member_id: &'a str,
    pub updates: PartialParticipantData,
}

#[derive(Serialize, Debug, Clone)]
pub struct UpdateMePayload {
    pub updates: PartialParticipantData,
    pub rid: i32,
}

#[derive(Serialize, Debug, Clone)]
pub struct UpdateAllPayload<'a> {
    pub updates: Vec<PartialParticipantDataForUpdate<'a>>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TournamentData {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub scheduled_for: DateTime<Utc>,
    pub description: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub text: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartialTournamentData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_for: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JoinSuccessPayload {
    pub data: TournamentData,
    pub member: TournamentRoomMember,
    pub participants: Vec<ParticipantData>,
    pub noauth: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantJoinedPayload {
    pub participant: ParticipantData,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantLeftPayload {
    pub member_id: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeaveSuccessPayload {
    pub message: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDataPayload {
    pub updates: PartialTournamentData,
}

#[derive(Deserialize, Debug)]
pub struct TypeEventPayload {
    pub character: char,
    pub rid: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEventPayload {
    pub correct_position: usize,
    pub current_position: usize,
    pub total_keystrokes: i32,
    pub rid: i32,
}
