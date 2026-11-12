use serde::{Deserialize, Serialize};

pub mod meeting;
pub mod user;

pub use meeting::{
    AgendaItemRecord, CreateAgendaItemRequest, CreateMeetingPromotionRequest, CreateMeetingRequest,
    CreateResolutionRequest, MeetingPromotionRecord, MeetingRecord, ResolutionRecord,
};
pub use user::{
    BootstrapRequest, DocumentRecord, JwtClaims, LoginRequest, TokenResponse, UserRecord,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiListResponse<T> {
    pub items: Vec<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
