use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::{
    api::AppError,
    app::AppState,
    models::ApiListResponse,
};

#[derive(Default, Deserialize)]
pub struct PublicMeetingsQuery {
    pub keyword: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct MeetingDocumentQuery {
    /// `download=1` 時以附件方式下載，否則在瀏覽器內檢視
    pub download: Option<String>,
}

/// 檢視 / 下載會議的原始文件。
pub async fn public_meeting_document(
    State(state): State<AppState>,
    Path(meeting_id): Path<String>,
    Query(query): Query<MeetingDocumentQuery>,
) -> Result<Response, AppError> {
    let Some(pool) = &state.pool else {
        return Err(AppError::new(StatusCode::NOT_FOUND, "此會議沒有原始文件"));
    };

    let meeting_uuid = uuid::Uuid::parse_str(&meeting_id)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "會議 ID 格式錯誤"))?;

    let row = sqlx::query(
        "SELECT title, meeting_title, meeting_date, source_file_name, source_file_path FROM meetings WHERE id = $1",
    )
    .bind(meeting_uuid)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "找不到此會議"))?;

    let title: String = row.try_get("title")?;
    let meeting_title: Option<String> = row.try_get("meeting_title")?;
    let meeting_date: Option<chrono::NaiveDate> = row.try_get("meeting_date")?;
    let file_path: Option<String> = row.try_get("source_file_path")?;
    let file_name: Option<String> = row.try_get("source_file_name")?;

    let resolved_path = file_path
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| {
            find_matching_storage_document(
                meeting_date,
                &title,
                meeting_title.as_deref(),
                file_name.as_deref(),
            )
        })
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "此會議沒有原始文件"))?;

    let bytes = tokio::fs::read(&resolved_path)
        .await
        .map_err(|_| AppError::new(StatusCode::NOT_FOUND, "原始文件已不存在"))?;

    let file_name = file_name
        .or_else(|| resolved_path.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "document.pdf".to_string());
    let content_type = content_type_for(&file_name);
    let disposition_type = if matches!(query.download.as_deref(), Some("1" | "true")) {
        "attachment"
    } else {
        "inline"
    };
    let ascii_fallback: String = file_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let disposition = format!(
        "{disposition_type}; filename=\"{ascii_fallback}\"; filename*=UTF-8''{}",
        percent_encode_utf8(&file_name)
    );

    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

fn content_type_for(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        "application/pdf"
    } else if lower.ends_with(".txt") || lower.ends_with(".md") {
        "text/plain; charset=utf-8"
    } else if lower.ends_with(".doc") {
        "application/msword"
    } else if lower.ends_with(".docx") {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    } else {
        "application/octet-stream"
    }
}

fn percent_encode_utf8(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// 若舊資料在資料庫尚未寫入 `source_file_path`，自動從 `storage/documents` 比對對應的原始檔案。
fn find_matching_storage_document(
    meeting_date: Option<chrono::NaiveDate>,
    title: &str,
    meeting_title: Option<&str>,
    source_file_name: Option<&str>,
) -> Option<std::path::PathBuf> {
    use chrono::Datelike;

    let candidate_dirs = [
        std::path::Path::new("./storage/documents"),
        std::path::Path::new("../storage/documents"),
    ];
    let dir = candidate_dirs.into_iter().find(|d| d.is_dir())?;

    if let Some(exact_name) = source_file_name.filter(|s| !s.trim().is_empty()) {
        let p = dir.join(exact_name);
        if p.is_file() {
            return Some(p);
        }
    }

    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .filter(|p| p.is_file())
        .collect();

    if let Some(date) = meeting_date {
        let year = date.year();
        let roc_year = year - 1911;
        let m = date.month();
        let d = date.day();

        let exact_date_patterns = [
            format!("{year:04}-{m:02}-{d:02}"),
            format!("{roc_year}-{m:02}-{d:02}"),
            format!("{roc_year}-{m}-{d}"),
            format!("{roc_year}_{m:02}_{d:02}"),
            format!("{roc_year}_{m}_{d}"),
            format!("{roc_year}.{m:02}.{d:02}"),
            format!("{roc_year}.{m}.{d}"),
            format!("{roc_year}年{m:02}月{d:02}日"),
            format!("{roc_year}年{m}月{d}日"),
        ];

        let mut best_match: Option<(i32, std::path::PathBuf)> = None;
        for path in &entries {
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let mut score = 0i32;
            if exact_date_patterns.iter().any(|pat| fname.contains(pat)) {
                score += 100;
            } else if fname.contains(&format!("{roc_year}年{m:02}月"))
                || fname.contains(&format!("{roc_year}年{m}月"))
            {
                score += 50;
            }
            if score > 0 {
                for kw in ["資訊", "電氣", "影音", "家電", "電磁相容", "太陽光電"] {
                    if (title.contains(kw) || meeting_title.is_some_and(|mt| mt.contains(kw)))
                        && fname.contains(kw)
                    {
                        score += 20;
                    }
                }
                if best_match.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                    best_match = Some((score, path.clone()));
                }
            }
        }
        if let Some((_, matched)) = best_match {
            return Some(matched);
        }
    }

    None
}

pub async fn public_meetings(
    State(state): State<AppState>,
    Query(query): Query<PublicMeetingsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let meetings = load_meetings_from_db(pool).await?;
        let meeting_promotions = load_meeting_promotions_from_db(pool, None).await?;
        let agenda_items = load_agenda_items_from_db(pool, None).await?;
        let resolutions = load_resolutions_from_db(pool, None).await?;
        let keyword = query.keyword.unwrap_or_default().trim().to_ascii_lowercase();

        let items = meetings
            .into_iter()
            .filter(|meeting| {
                keyword.is_empty()
                    || meeting_matches_keyword(meeting, &keyword)
                    || meeting_promotions.values().any(|promotion| {
                        promotion.meeting_id == meeting.id
                            && meeting_promotion_matches_keyword(promotion, &keyword)
                    })
                    || agenda_items.values().any(|agenda_item| {
                        agenda_item.meeting_id == meeting.id
                            && agenda_item_matches_keyword(agenda_item, &keyword)
                    })
                    || resolutions.values().any(|resolution| {
                        resolution.meeting_id == meeting.id
                            && resolution_matches_keyword(resolution, &keyword)
                    })
            })
            .collect::<Vec<_>>();

        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let meetings = state.meetings.read().unwrap();
    let meeting_promotions = state.meeting_promotions.read().unwrap();
    let agenda_items = state.agenda_items.read().unwrap();
    let resolutions = state.resolutions.read().unwrap();
    let keyword = query.keyword.unwrap_or_default().trim().to_ascii_lowercase();
    let mut items = meetings
        .values()
        .filter(|meeting| {
            keyword.is_empty()
                || meeting_matches_keyword(meeting, &keyword)
                || meeting_promotions.values().any(|promotion| {
                    promotion.meeting_id == meeting.id
                        && meeting_promotion_matches_keyword(promotion, &keyword)
                })
                || agenda_items.values().any(|agenda_item| {
                    agenda_item.meeting_id == meeting.id
                        && agenda_item_matches_keyword(agenda_item, &keyword)
                })
                || resolutions.values().any(|resolution| {
                    resolution.meeting_id == meeting.id
                        && resolution_matches_keyword(resolution, &keyword)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_meetings_for_display(&mut items);

    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn public_agenda_items(
    State(state): State<AppState>,
    Path(meeting_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let mut items = load_agenda_items_from_db(pool, Some(&meeting_id))
            .await?
            .into_values()
            .collect::<Vec<_>>();
        sort_agenda_items_for_display(&mut items);
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let agenda_items = state.agenda_items.read().unwrap();
    let mut items = agenda_items
        .values()
        .filter(|agenda_item| agenda_item.meeting_id == meeting_id)
        .cloned()
        .collect::<Vec<_>>();
    sort_agenda_items_for_display(&mut items);

    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn public_resolutions(
    State(state): State<AppState>,
    Path(meeting_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let mut items = load_resolutions_from_db(pool, Some(&meeting_id))
            .await?
            .into_values()
            .collect::<Vec<_>>();
        sort_resolutions_for_display(&mut items);
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let resolutions = state.resolutions.read().unwrap();
    let mut items = resolutions
        .values()
        .filter(|resolution| resolution.meeting_id == meeting_id)
        .cloned()
        .collect::<Vec<_>>();
    sort_resolutions_for_display(&mut items);

    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn public_meeting_promotions(
    State(state): State<AppState>,
    Path(meeting_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let mut items = load_meeting_promotions_from_db(pool, Some(&meeting_id))
            .await?
            .into_values()
            .collect::<Vec<_>>();
        sort_promotions_for_display(&mut items);
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let meeting_promotions = state.meeting_promotions.read().unwrap();
    let mut items = meeting_promotions
        .values()
        .filter(|promotion| promotion.meeting_id == meeting_id)
        .cloned()
        .collect::<Vec<_>>();
    sort_promotions_for_display(&mut items);

    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

fn sort_meetings_for_display(items: &mut [crate::models::MeetingRecord]) {
    items.sort_by(|a, b| {
        b.meeting_date
            .cmp(&a.meeting_date)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn compare_numeric_or_text(left: &str, right: &str) -> std::cmp::Ordering {
    let left_value = left.trim();
    let right_value = right.trim();

    match (left_value.parse::<i64>(), right_value.parse::<i64>()) {
        (Ok(left_num), Ok(right_num)) => left_num.cmp(&right_num),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => left_value.cmp(right_value),
    }
}

fn sort_agenda_items_for_display(items: &mut [crate::models::AgendaItemRecord]) {
    items.sort_by(|a, b| {
        compare_numeric_or_text(&a.item_no, &b.item_no)
            .then_with(|| {
                a.created_at
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.created_at.as_deref().unwrap_or_default())
            })
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn sort_promotions_for_display(items: &mut [crate::models::MeetingPromotionRecord]) {
    items.sort_by(|a, b| {
        a.promotion_no
            .cmp(&b.promotion_no)
            .then_with(|| {
                a.created_at
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.created_at.as_deref().unwrap_or_default())
            })
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn sort_resolutions_for_display(items: &mut [crate::models::ResolutionRecord]) {
    items.sort_by(|a, b| {
        compare_numeric_or_text(&a.item_no, &b.item_no)
            .then_with(|| {
                a.created_at
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.created_at.as_deref().unwrap_or_default())
            })
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn matches_text(value: &str, keyword: &str) -> bool {
    value.to_ascii_lowercase().contains(keyword)
}

fn matches_optional(value: Option<&str>, keyword: &str) -> bool {
    value.is_some_and(|entry| matches_text(entry, keyword))
}

fn meeting_matches_keyword(meeting: &crate::models::MeetingRecord, keyword: &str) -> bool {
    matches_text(&meeting.id, keyword)
        || matches_text(&meeting.meeting_no, keyword)
        || matches_text(&meeting.title, keyword)
        || matches_text(&meeting.summary, keyword)
        || matches_text(&meeting.meeting_date, keyword)
        || matches_optional(meeting.published_at.as_deref(), keyword)
        || matches_optional(meeting.meeting_time.as_deref(), keyword)
        || matches_optional(meeting.meeting_title.as_deref(), keyword)
        || matches_optional(meeting.chairperson.as_deref(), keyword)
        || matches_optional(meeting.attendees.as_deref(), keyword)
        || matches_optional(meeting.meeting_location.as_deref(), keyword)
        || matches_optional(meeting.recorder.as_deref(), keyword)
}

fn agenda_item_matches_keyword(agenda_item: &crate::models::AgendaItemRecord, keyword: &str) -> bool {
    matches_text(&agenda_item.id, keyword)
        || matches_text(&agenda_item.meeting_id, keyword)
        || matches_text(&agenda_item.item_no, keyword)
        || matches_text(&agenda_item.agenda_item, keyword)
        || matches_optional(agenda_item.notes.as_deref(), keyword)
        || matches_optional(agenda_item.created_at.as_deref(), keyword)
}

fn meeting_promotion_matches_keyword(
    promotion: &crate::models::MeetingPromotionRecord,
    keyword: &str,
) -> bool {
    matches_text(&promotion.id, keyword)
        || matches_text(&promotion.meeting_id, keyword)
        || matches_text(&promotion.promotion_no.to_string(), keyword)
        || matches_text(&promotion.content, keyword)
        || matches_optional(promotion.notes.as_deref(), keyword)
        || matches_optional(promotion.created_at.as_deref(), keyword)
}

fn resolution_matches_keyword(resolution: &crate::models::ResolutionRecord, keyword: &str) -> bool {
    matches_text(&resolution.id, keyword)
        || matches_text(&resolution.meeting_id, keyword)
        || matches_optional(resolution.agenda_item_id.as_deref(), keyword)
        || matches_text(&resolution.item_no, keyword)
        || matches_text(&resolution.title, keyword)
        || matches_text(&resolution.content, keyword)
        || matches_optional(resolution.resolution.as_deref(), keyword)
        || matches_text(&resolution.result, keyword)
        || matches_optional(resolution.created_at.as_deref(), keyword)
}

async fn load_meetings_from_db(pool: &sqlx::PgPool) -> Result<Vec<crate::models::MeetingRecord>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id, meeting_no, title, summary, meeting_date, published_at, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, source_file_name
        FROM meetings
        ORDER BY meeting_date DESC NULLS LAST, created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let title: String = row.try_get("title")?;
            let meeting_title: Option<String> = row.try_get("meeting_title")?;
            let raw_date: Option<chrono::NaiveDate> = row.try_get("meeting_date")?;
            let db_file_name: Option<String> = row.try_get("source_file_name")?;
            let resolved_file_name = db_file_name.or_else(|| {
                find_matching_storage_document(raw_date, &title, meeting_title.as_deref(), None)
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            });
            Ok(crate::models::MeetingRecord {
                id: row.try_get::<uuid::Uuid, _>("id")?.to_string(),
                meeting_no: row.try_get::<Option<String>, _>("meeting_no")?.unwrap_or_default(),
                title,
                summary: row.try_get::<Option<String>, _>("summary")?.unwrap_or_default(),
                meeting_date: raw_date.map(|value| value.to_string()).unwrap_or_default(),
                published_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("published_at")?
                    .map(|value| value.to_rfc3339()),
                meeting_time: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("meeting_time")?
                    .map(|value| value.to_rfc3339()),
                meeting_title,
                chairperson: row.try_get("chairperson")?,
                attendees: row.try_get("attendees")?,
                meeting_location: row.try_get("meeting_location")?,
                recorder: row.try_get("recorder")?,
                source_file_name: resolved_file_name,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(AppError::from)
}

async fn load_meeting_promotions_from_db(
    pool: &sqlx::PgPool,
    meeting_id: Option<&str>,
) -> Result<std::collections::HashMap<String, crate::models::MeetingPromotionRecord>, AppError> {
    let rows = if let Some(id) = meeting_id {
        sqlx::query(
            r#"
            SELECT id, meeting_id, promotion_no, content, notes, created_at
            FROM meeting_promotions
            WHERE meeting_id::text = $1
            ORDER BY promotion_no ASC, created_at ASC
            "#,
        )
        .bind(id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, meeting_id, promotion_no, content, notes, created_at
            FROM meeting_promotions
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let id = row.try_get::<uuid::Uuid, _>("id")?.to_string();
        map.insert(
            id.clone(),
            crate::models::MeetingPromotionRecord {
                id,
                meeting_id: row.try_get::<uuid::Uuid, _>("meeting_id")?.to_string(),
                promotion_no: row.try_get("promotion_no")?,
                content: row.try_get("content")?,
                notes: row.try_get("notes")?,
                created_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?
                    .map(|value| value.to_rfc3339()),
            },
        );
    }

    Ok(map)
}

async fn load_agenda_items_from_db(
    pool: &sqlx::PgPool,
    meeting_id: Option<&str>,
) -> Result<std::collections::HashMap<String, crate::models::AgendaItemRecord>, AppError> {
    let rows = if let Some(id) = meeting_id {
        sqlx::query(
            r#"
            SELECT id, meeting_id, item_no, agenda_item, notes, created_at
            FROM agenda_items
            WHERE meeting_id::text = $1
            ORDER BY item_no ASC, created_at ASC
            "#,
        )
        .bind(id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, meeting_id, item_no, agenda_item, notes, created_at
            FROM agenda_items
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let id = row.try_get::<uuid::Uuid, _>("id")?.to_string();
        map.insert(
            id.clone(),
            crate::models::AgendaItemRecord {
                id,
                meeting_id: row.try_get::<uuid::Uuid, _>("meeting_id")?.to_string(),
                item_no: row.try_get("item_no")?,
                agenda_item: row.try_get("agenda_item")?,
                notes: row.try_get("notes")?,
                created_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?
                    .map(|value| value.to_rfc3339()),
            },
        );
    }

    Ok(map)
}

async fn load_resolutions_from_db(
    pool: &sqlx::PgPool,
    meeting_id: Option<&str>,
) -> Result<std::collections::HashMap<String, crate::models::ResolutionRecord>, AppError> {
    let rows = if let Some(id) = meeting_id {
        sqlx::query(
            r#"
            SELECT id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at
            FROM resolutions
            WHERE meeting_id::text = $1
            ORDER BY
                CASE
                    WHEN item_no ~ '^[0-9]+$' THEN CAST(item_no AS integer)
                    ELSE NULL
                END ASC NULLS LAST,
                item_no ASC,
                created_at ASC
            "#,
        )
        .bind(id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at
            FROM resolutions
            ORDER BY
                CASE
                    WHEN item_no ~ '^[0-9]+$' THEN CAST(item_no AS integer)
                    ELSE NULL
                END ASC NULLS LAST,
                item_no ASC,
                created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let id = row.try_get::<uuid::Uuid, _>("id")?.to_string();
        map.insert(
            id.clone(),
            crate::models::ResolutionRecord {
                id,
                meeting_id: row.try_get::<uuid::Uuid, _>("meeting_id")?.to_string(),
                agenda_item_id: row
                    .try_get::<Option<uuid::Uuid>, _>("agenda_item_id")?
                    .map(|value| value.to_string()),
                item_no: row.try_get("item_no")?,
                title: row.try_get("title")?,
                content: row.try_get("content")?,
                resolution: row.try_get("resolution")?,
                result: row.try_get("result")?,
                created_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?
                    .map(|value| value.to_rfc3339()),
            },
        );
    }

    Ok(map)
}
