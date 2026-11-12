use anyhow::Result;
use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    app::AppState,
    models::{
        AgendaItemRecord, MeetingPromotionRecord, MeetingRecord, ResolutionRecord, UserRecord,
    },
    services::auth::hash_password,
};

pub async fn seed_demo_data(state: &AppState) -> Result<()> {
    if let Some(pool) = &state.pool {
        seed_demo_data_postgres(pool).await?;
    }

    seed_demo_data_memory(state)
}

fn seed_demo_data_memory(state: &AppState) -> Result<()> {
    // let mut users = state.users.write().unwrap();
    // if users.is_empty() {
    //     let password_hash = hash_password("Admin123!")?;
    //     users.insert(
    //         "admin@consensus.test".to_string(),
    //         UserRecord {
    //             id: "a78cf4cc-7a67-4630-baa4-5554c2dc28be".to_string(),
    //             email: "admin@consensus.test".to_string(),
    //             display_name: "系統管理員".to_string(),
    //             password_hash,
    //             is_active: true,
    //             created_at: Utc::now().to_rfc3339(),
    //         },
    //     );
    // }

    // let mut meetings = state.meetings.write().unwrap();
    // if meetings.is_empty() {
    //     meetings.insert(
    //         "3bd47428-63dc-4824-a0cc-90816a3f08f0".to_string(),
    //         MeetingRecord {
    //             id: "3bd47428-63dc-4824-a0cc-90816a3f08f0".to_string(),
    //             meeting_no: "2026-01".to_string(),
    //             title: "2026 年第一季目標對齊會議".to_string(),
    //             summary: "檢視各團隊本季重點並確認執行節奏與資源配置。".to_string(),
    //             meeting_date: "2026-01-15".to_string(),
    //             published_at: Some(Utc::now().to_rfc3339()),
    //             meeting_time: Some("2026-01-15T09:00:00+08:00".to_string()),
    //             meeting_title: Some("2026 年第一季目標對齊會議".to_string()),
    //             chairperson: Some("陳怡君".to_string()),
    //             attendees: Some("產品部、研發部、營運部".to_string()),
    //             meeting_location: Some("第一會議室".to_string()),
    //             recorder: Some("林柏宏".to_string()),
    //             source_file_name: None,
    //         },
    //     );
    //     meetings.insert(
    //         "b4406e2d-0f54-42c2-aafc-70596853ca5c".to_string(),
    //         MeetingRecord {
    //             id: "b4406e2d-0f54-42c2-aafc-70596853ca5c".to_string(),
    //             meeting_no: "2026-02".to_string(),
    //             title: "年度預算審議會議".to_string(),
    //             summary: "審議下一期營運預算與人力配置，確認執行優先順序。".to_string(),
    //             meeting_date: "2026-02-12".to_string(),
    //             published_at: Some(Utc::now().to_rfc3339()),
    //             meeting_time: Some("2026-02-12T14:30:00+08:00".to_string()),
    //             meeting_title: Some("年度預算審議會議".to_string()),
    //             chairperson: Some("吳志明".to_string()),
    //             attendees: Some("財務部、營運部、管理層".to_string()),
    //             meeting_location: Some("董事會會議室".to_string()),
    //             recorder: Some("蔡雅婷".to_string()),
    //             source_file_name: None,
    //         },
    //     );
    // }

    // let mut agenda_items = state.agenda_items.write().unwrap();
    // if agenda_items.is_empty() {
    //     let meeting_id = "3bd47428-63dc-4824-a0cc-90816a3f08f0".to_string();
    //     agenda_items.insert(
    //         "f6ed3389-8f52-4f2a-ba93-8f398da3bfd8".to_string(),
    //         AgendaItemRecord {
    //             id: "f6ed3389-8f52-4f2a-ba93-8f398da3bfd8".to_string(),
    //             meeting_id: meeting_id.clone(),
    //             item_no: "A.1".to_string(),
    //             agenda_item: "檢視第一季交付計畫".to_string(),
    //             notes: Some("優先處理對客戶影響較高的里程碑".to_string()),
    //             created_at: Some(Utc::now().to_rfc3339()),
    //         },
    //     );
    //     agenda_items.insert(
    //         "c95c042f-2ef8-4916-8a41-a1a5008e0d7c".to_string(),
    //         AgendaItemRecord {
    //             id: "c95c042f-2ef8-4916-8a41-a1a5008e0d7c".to_string(),
    //             meeting_id: meeting_id.clone(),
    //             item_no: "B.2".to_string(),
    //             agenda_item: "確認客服升級通報流程".to_string(),
    //             notes: Some("需與值班輪值制度一併調整".to_string()),
    //             created_at: Some(Utc::now().to_rfc3339()),
    //         },
    //     );

    //     let meeting_id_two = "b4406e2d-0f54-42c2-aafc-70596853ca5c".to_string();
    //     agenda_items.insert(
    //         "f7397e98-26be-4eb6-b5bc-9b8f6f1847f4".to_string(),
    //         AgendaItemRecord {
    //             id: "f7397e98-26be-4eb6-b5bc-9b8f6f1847f4".to_string(),
    //             meeting_id: meeting_id_two,
    //             item_no: "C.1".to_string(),
    //             agenda_item: "審議年度預算分配".to_string(),
    //             notes: Some("各部門人力編制需於本月定案".to_string()),
    //             created_at: Some(Utc::now().to_rfc3339()),
    //         },
    //     );
    // }

    // let mut meeting_promotions = state.meeting_promotions.write().unwrap();
    // if meeting_promotions.is_empty() {
    //     meeting_promotions.insert(
    //         "1f4e2b0a-1f0d-466e-85ee-f009159ecd8c".to_string(),
    //         MeetingPromotionRecord {
    //             id: "1f4e2b0a-1f0d-466e-85ee-f009159ecd8c".to_string(),
    //             meeting_id: "3bd47428-63dc-4824-a0cc-90816a3f08f0".to_string(),
    //             promotion_no: 1,
    //             content: "請各部門每週五下班前更新專案進度至協作平台。".to_string(),
    //             notes: Some("由各部門窗口彙整後提交".to_string()),
    //             created_at: Some(Utc::now().to_rfc3339()),
    //         },
    //     );
    //     meeting_promotions.insert(
    //         "78f9ddff-769f-44d5-b463-2d0e4621adba".to_string(),
    //         MeetingPromotionRecord {
    //             id: "78f9ddff-769f-44d5-b463-2d0e4621adba".to_string(),
    //             meeting_id: "b4406e2d-0f54-42c2-aafc-70596853ca5c".to_string(),
    //             promotion_no: 1,
    //             content: "預算執行需遵循採購流程，超過門檻需先完成簽核。".to_string(),
    //             notes: Some("財務部每月抽查".to_string()),
    //             created_at: Some(Utc::now().to_rfc3339()),
    //         },
    //     );
    // }

    // let mut resolutions = state.resolutions.write().unwrap();
    // if resolutions.is_empty() {
    //     let meeting_id = "3bd47428-63dc-4824-a0cc-90816a3f08f0".to_string();
    //     resolutions.insert(
    //         "ec176ee7-4e62-4f53-bf36-c3296c775f70".to_string(),
    //         ResolutionRecord {
    //             id: "ec176ee7-4e62-4f53-bf36-c3296c775f70".to_string(),
    //             meeting_id: meeting_id.clone(),
    //             agenda_item_id: Some("f6ed3389-8f52-4f2a-ba93-8f398da3bfd8".to_string()),
    //             item_no: "A.1".to_string(),
    //             title: "第一季交付計畫".to_string(),
    //             content: "確認本季專案待辦清單與跨團隊資源分配方式。".to_string(),
    //             resolution: Some("通過第一季交付計畫與資源配置，依排程執行。".to_string()),
    //             result: "通過".to_string(),
    //             created_at: None,
    //         },
    //     );
    //     resolutions.insert(
    //         "2e7d9dc6-74f9-4571-ae4e-a8623ccf22c8".to_string(),
    //         ResolutionRecord {
    //             id: "2e7d9dc6-74f9-4571-ae4e-a8623ccf22c8".to_string(),
    //             meeting_id: meeting_id.clone(),
    //             agenda_item_id: Some("c95c042f-2ef8-4916-8a41-a1a5008e0d7c".to_string()),
    //             item_no: "B.2".to_string(),
    //             title: "客服升級通報機制".to_string(),
    //             content: "針對影響服務穩定性的事件，建立明確升級通報路徑。".to_string(),
    //             resolution: Some("請客服與 SRE 於兩週內提出新版升級流程後再議。".to_string()),
    //             result: "緩議".to_string(),
    //             created_at: None,
    //         },
    //     );

    //     let meeting_id_two = "b4406e2d-0f54-42c2-aafc-70596853ca5c".to_string();
    //     resolutions.insert(
    //         "4ea64f39-2536-49dc-a40b-ea756586ae0e".to_string(),
    //         ResolutionRecord {
    //             id: "4ea64f39-2536-49dc-a40b-ea756586ae0e".to_string(),
    //             meeting_id: meeting_id_two.clone(),
    //             agenda_item_id: Some("f7397e98-26be-4eb6-b5bc-9b8f6f1847f4".to_string()),
    //             item_no: "C.1".to_string(),
    //             title: "年度預算核定".to_string(),
    //             content: "核定本期預算，確保各單位可依時程推進執行計畫。".to_string(),
    //             resolution: Some("通過年度預算，並要求各部門每月提交執行追蹤報告。".to_string()),
    //             result: "通過".to_string(),
    //             created_at: None,
    //         },
    //     );
    // }

    Ok(())
}

async fn seed_demo_data_postgres(pool: &PgPool) -> Result<()> {
    let password_hash = hash_password("Admin123!")?;
    let now = Utc::now();
    let meeting_001_time =
        chrono::DateTime::parse_from_rfc3339("2026-01-15T09:00:00+08:00")?.with_timezone(&Utc);
    let meeting_002_time =
        chrono::DateTime::parse_from_rfc3339("2026-02-12T14:30:00+08:00")?.with_timezone(&Utc);

    sqlx::query(
        r#"
        INSERT INTO users (id, email, display_name, password_hash, is_active, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (email) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("a78cf4cc-7a67-4630-baa4-5554c2dc28be")?)
    .bind("admin@consensus.test")
    .bind("系統管理員")
    .bind(password_hash)
    .bind(true)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO meetings
            (id, meeting_no, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, title, summary, meeting_date, published_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind("2026-01")
    .bind(meeting_001_time)
    .bind("2026 年第一季目標對齊會議")
    .bind("陳怡君")
    .bind("產品部、研發部、營運部")
    .bind("第一會議室")
    .bind("林柏宏")
    .bind("2026 年第一季目標對齊會議")
    .bind("檢視各團隊本季重點並確認執行節奏與資源配置。")
    .bind(NaiveDate::from_ymd_opt(2026, 1, 15))
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO meetings
            (id, meeting_no, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, title, summary, meeting_date, published_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("b4406e2d-0f54-42c2-aafc-70596853ca5c")?)
    .bind("2026-02")
    .bind(meeting_002_time)
    .bind("年度預算審議會議")
    .bind("吳志明")
    .bind("財務部、營運部、管理層")
    .bind("董事會會議室")
    .bind("蔡雅婷")
    .bind("年度預算審議會議")
    .bind("審議下一期營運預算與人力配置，確認執行優先順序。")
    .bind(NaiveDate::from_ymd_opt(2026, 2, 12))
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO meeting_promotions (id, meeting_id, promotion_no, content, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (meeting_id, promotion_no) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("1f4e2b0a-1f0d-466e-85ee-f009159ecd8c")?)
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind(1_i32)
    .bind("請各部門每週五下班前更新專案進度至協作平台。")
    .bind(Some("由各部門窗口彙整後提交"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO meeting_promotions (id, meeting_id, promotion_no, content, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (meeting_id, promotion_no) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("78f9ddff-769f-44d5-b463-2d0e4621adba")?)
    .bind(Uuid::parse_str("b4406e2d-0f54-42c2-aafc-70596853ca5c")?)
    .bind(1_i32)
    .bind("預算執行需遵循採購流程，超過門檻需先完成簽核。")
    .bind(Some("財務部每月抽查"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO agenda_items (id, meeting_id, item_no, agenda_item, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (meeting_id, item_no) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("f6ed3389-8f52-4f2a-ba93-8f398da3bfd8")?)
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind("A.1")
    .bind("檢視第一季交付計畫")
    .bind(Some("優先處理對客戶影響較高的里程碑"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO agenda_items (id, meeting_id, item_no, agenda_item, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (meeting_id, item_no) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("c95c042f-2ef8-4916-8a41-a1a5008e0d7c")?)
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind("B.2")
    .bind("確認客服升級通報流程")
    .bind(Some("需與值班輪值制度一併調整"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO agenda_items (id, meeting_id, item_no, agenda_item, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (meeting_id, item_no) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("f7397e98-26be-4eb6-b5bc-9b8f6f1847f4")?)
    .bind(Uuid::parse_str("b4406e2d-0f54-42c2-aafc-70596853ca5c")?)
    .bind("C.1")
    .bind("審議年度預算分配")
    .bind(Some("各部門人力編制需於本月定案"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO resolutions
            (id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (agenda_item_id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("ec176ee7-4e62-4f53-bf36-c3296c775f70")?)
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind(Uuid::parse_str("f6ed3389-8f52-4f2a-ba93-8f398da3bfd8")?)
    .bind("A.1")
    .bind("第一季交付計畫")
    .bind("確認本季專案待辦清單與跨團隊資源分配方式。")
    .bind(Some("通過第一季交付計畫與資源配置，依排程執行。"))
    .bind("通過")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO resolutions
            (id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (agenda_item_id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("2e7d9dc6-74f9-4571-ae4e-a8623ccf22c8")?)
    .bind(Uuid::parse_str("3bd47428-63dc-4824-a0cc-90816a3f08f0")?)
    .bind(Uuid::parse_str("c95c042f-2ef8-4916-8a41-a1a5008e0d7c")?)
    .bind("B.2")
    .bind("客服升級通報機制")
    .bind("針對影響服務穩定性的事件，建立明確升級通報路徑。")
    .bind(Some("請客服與 SRE 於兩週內提出新版升級流程後再議。"))
    .bind("緩議")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO resolutions
            (id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (agenda_item_id) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("4ea64f39-2536-49dc-a40b-ea756586ae0e")?)
    .bind(Uuid::parse_str("b4406e2d-0f54-42c2-aafc-70596853ca5c")?)
    .bind(Uuid::parse_str("f7397e98-26be-4eb6-b5bc-9b8f6f1847f4")?)
    .bind("C.1")
    .bind("年度預算核定")
    .bind("核定本期預算，確保各單位可依時程推進執行計畫。")
    .bind(Some("通過年度預算，並要求各部門每月提交執行追蹤報告。"))
    .bind("通過")
    .execute(pool)
    .await?;

    Ok(())
}
