use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingRecord {
    pub id: String,
    pub meeting_no: String,
    pub title: String,
    pub summary: String,
    pub meeting_date: String,
    pub published_at: Option<String>,
    pub meeting_time: Option<String>,
    pub meeting_title: Option<String>,
    pub chairperson: Option<String>,
    pub attendees: Option<String>,
    pub meeting_location: Option<String>,
    pub recorder: Option<String>,
    /// 原始會議文件檔名（若有），可透過 `/api/public/meetings/{id}/document` 檢視
    #[serde(default)]
    pub source_file_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateMeetingRequest {
    pub meeting_no: String,
    pub title: String,
    pub summary: String,
    pub meeting_date: String,
    pub published_at: Option<String>,
    pub meeting_time: Option<String>,
    pub meeting_title: Option<String>,
    pub chairperson: Option<String>,
    pub attendees: Option<String>,
    pub meeting_location: Option<String>,
    pub recorder: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingPromotionRecord {
    pub id: String,
    pub meeting_id: String,
    pub promotion_no: i32,
    pub content: String,
    pub notes: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateMeetingPromotionRequest {
    pub meeting_id: String,
    pub promotion_no: i32,
    pub content: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgendaItemRecord {
    pub id: String,
    pub meeting_id: String,
    pub item_no: String,
    pub agenda_item: String,
    pub notes: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAgendaItemRequest {
    pub meeting_id: String,
    pub item_no: String,
    pub agenda_item: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolutionRecord {
    pub id: String,
    pub meeting_id: String,
    pub agenda_item_id: Option<String>,
    pub item_no: String,
    pub title: String,
    pub content: String,
    pub resolution: Option<String>,
    pub result: String,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateResolutionRequest {
    pub meeting_id: Option<String>,
    pub agenda_item_id: Option<String>,
    pub item_no: String,
    pub title: String,
    pub content: String,
    pub resolution: Option<String>,
    pub result: String,
}
