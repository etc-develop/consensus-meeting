use crate::api::admin::{
    admin_documents, admin_meetings, admin_resolutions, admin_users, create_meeting,
    create_resolution, patch_meeting, upload_document,
};
use crate::app::AppState; // 引入你的 AppState 型別

use axum::{
    Router,
    routing::{get, patch},
};
pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(admin_users))
        .route(
            "/api/admin/meetings",
            get(admin_meetings).post(create_meeting),
        )
        .route("/api/admin/meetings/{meeting_id}", patch(patch_meeting))
        .route(
            "/api/admin/resolutions",
            get(admin_resolutions).post(create_resolution),
        )
        .route(
            "/api/admin/documents",
            get(admin_documents).post(upload_document),
        )
}
