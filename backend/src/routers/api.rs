use crate::api::{
    auth::{bootstrap_admin, current_user, login},
    public::{
        public_agenda_items, public_meeting_document, public_meeting_promotions, public_meetings,
        public_resolutions,
    },
};
use crate::app::AppState; // 引入你的 AppState 型別
use axum::{
    Router,
    routing::{get, post},
};

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(crate::api::health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/bootstrap", post(bootstrap_admin))
        .route("/api/auth/me", get(current_user))
        .route("/api/public/meetings", get(public_meetings))
        .route(
            "/api/public/meetings/{meeting_id}/agenda-items",
            get(public_agenda_items),
        )
        .route(
            "/api/public/meetings/{meeting_id}/resolutions",
            get(public_resolutions),
        )
        .route(
            "/api/public/meetings/{meeting_id}/promotions",
            get(public_meeting_promotions),
        )
        .route(
            "/api/public/meetings/{meeting_id}/document",
            get(public_meeting_document),
        )
}
