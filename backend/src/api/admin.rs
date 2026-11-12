use std::fs;

use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::Multipart;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::Row;

use crate::{
    api::AppError,
    app::AppState,
    models::{ApiListResponse, CreateMeetingRequest, CreateResolutionRequest, DocumentRecord},
};

pub async fn admin_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let rows = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, is_active, created_at
            FROM users
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?;
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(crate::models::UserRecord {
                    id: row.try_get::<uuid::Uuid, _>("id")?.to_string(),
                    email: row.try_get("email")?,
                    display_name: row.try_get("display_name")?,
                    password_hash: row.try_get("password_hash")?,
                    is_active: row.try_get("is_active")?,
                    created_at: row
                        .try_get::<DateTime<Utc>, _>("created_at")?
                        .to_rfc3339(),
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let users = state.users.read().unwrap();
    let mut items = users.values().cloned().collect::<Vec<_>>();
    items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn admin_meetings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let rows = sqlx::query(
            r#"
            SELECT id, meeting_no, title, summary, meeting_date, published_at, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, source_file_name
            FROM meetings
            ORDER BY meeting_date DESC NULLS LAST, created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(crate::models::MeetingRecord {
                    id: row.try_get::<uuid::Uuid, _>("id")?.to_string(),
                    meeting_no: row.try_get::<Option<String>, _>("meeting_no")?.unwrap_or_default(),
                    title: row.try_get("title")?,
                    summary: row.try_get::<Option<String>, _>("summary")?.unwrap_or_default(),
                    meeting_date: row
                        .try_get::<Option<NaiveDate>, _>("meeting_date")?
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    published_at: row
                        .try_get::<Option<DateTime<Utc>>, _>("published_at")?
                        .map(|value| value.to_rfc3339()),
                    meeting_time: row
                        .try_get::<Option<DateTime<Utc>>, _>("meeting_time")?
                        .map(|value| value.to_rfc3339()),
                    meeting_title: row.try_get("meeting_title")?,
                    chairperson: row.try_get("chairperson")?,
                    attendees: row.try_get("attendees")?,
                    meeting_location: row.try_get("meeting_location")?,
                    recorder: row.try_get("recorder")?,
                    source_file_name: row.try_get("source_file_name")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let meetings = state.meetings.read().unwrap();
    let mut items = meetings.values().cloned().collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.meeting_date
            .cmp(&a.meeting_date)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn create_meeting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateMeetingRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let id = uuid::Uuid::new_v4();
        let meeting_date = parse_date_optional(&payload.meeting_date)?;
        let meeting_time = parse_datetime_optional(payload.meeting_time.as_deref())?;
        let published_at = parse_datetime_optional(payload.published_at.as_deref())?;
        sqlx::query(
            r#"
            INSERT INTO meetings
                (id, meeting_no, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, title, summary, meeting_date, published_at, created_at, updated_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
            "#,
        )
        .bind(id)
        .bind(&payload.meeting_no)
        .bind(meeting_time)
        .bind(&payload.meeting_title)
        .bind(&payload.chairperson)
        .bind(&payload.attendees)
        .bind(&payload.meeting_location)
        .bind(&payload.recorder)
        .bind(&payload.title)
        .bind(&payload.summary)
        .bind(meeting_date)
        .bind(published_at)
        .execute(pool)
        .await?;

        let meeting = crate::models::MeetingRecord {
            id: id.to_string(),
            meeting_no: payload.meeting_no,
            title: payload.title,
            summary: payload.summary,
            meeting_date: payload.meeting_date,
            published_at: payload.published_at,
            meeting_time: payload.meeting_time,
            meeting_title: payload.meeting_title,
            chairperson: payload.chairperson,
            attendees: payload.attendees,
            meeting_location: payload.meeting_location,
            recorder: payload.recorder,
            source_file_name: None,
        };
        return Ok((StatusCode::CREATED, Json(meeting)));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let meeting = crate::models::MeetingRecord {
        id: id.clone(),
        meeting_no: payload.meeting_no,
        title: payload.title,
        summary: payload.summary,
        meeting_date: payload.meeting_date,
        published_at: payload.published_at,
        meeting_time: payload.meeting_time,
        meeting_title: payload.meeting_title,
        chairperson: payload.chairperson,
        attendees: payload.attendees,
        meeting_location: payload.meeting_location,
        recorder: payload.recorder,
        source_file_name: None,
    };

    state
        .meetings
        .write()
        .unwrap()
        .insert(id.clone(), meeting.clone());
    Ok((StatusCode::CREATED, Json(meeting)))
}

pub async fn patch_meeting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(meeting_id): Path<String>,
    Json(payload): Json<CreateMeetingRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let meeting_uuid = uuid::Uuid::parse_str(&meeting_id).map_err(|_| {
            AppError::new(
                StatusCode::BAD_REQUEST,
                format!("invalid meeting id '{}'", meeting_id),
            )
        })?;

        let meeting_date = parse_date_optional(&payload.meeting_date)?;
        let meeting_time = parse_datetime_optional(payload.meeting_time.as_deref())?;
        let published_at = parse_datetime_optional(payload.published_at.as_deref())?;
        let result = sqlx::query(
            r#"
            UPDATE meetings
            SET meeting_no = $2,
                title = $3,
                summary = $4,
                meeting_date = $5,
                published_at = $6,
                meeting_time = $7,
                meeting_title = $8,
                chairperson = $9,
                attendees = $10,
                meeting_location = $11,
                recorder = $12,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(meeting_uuid)
        .bind(&payload.meeting_no)
        .bind(&payload.title)
        .bind(&payload.summary)
        .bind(meeting_date)
        .bind(published_at)
        .bind(meeting_time)
        .bind(&payload.meeting_title)
        .bind(&payload.chairperson)
        .bind(&payload.attendees)
        .bind(&payload.meeting_location)
        .bind(&payload.recorder)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::new(
                StatusCode::NOT_FOUND,
                format!("meeting '{}' not found", meeting_id),
            ));
        }

        let meeting = crate::models::MeetingRecord {
            id: meeting_id,
            meeting_no: payload.meeting_no,
            title: payload.title,
            summary: payload.summary,
            meeting_date: payload.meeting_date,
            published_at: payload.published_at,
            meeting_time: payload.meeting_time,
            meeting_title: payload.meeting_title,
            chairperson: payload.chairperson,
            attendees: payload.attendees,
            meeting_location: payload.meeting_location,
            recorder: payload.recorder,
            source_file_name: None,
        };
        return Ok((StatusCode::OK, Json(meeting)));
    }

    let mut meetings = state.meetings.write().unwrap();
    let meeting = meetings.get_mut(&meeting_id).ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_FOUND,
            format!("meeting '{}' not found", meeting_id),
        )
    })?;

    meeting.meeting_no = payload.meeting_no;
    meeting.title = payload.title;
    meeting.summary = payload.summary;
    meeting.meeting_date = payload.meeting_date;
    meeting.published_at = payload.published_at;
    meeting.meeting_time = payload.meeting_time;
    meeting.meeting_title = payload.meeting_title;
    meeting.chairperson = payload.chairperson;
    meeting.attendees = payload.attendees;
    meeting.meeting_location = payload.meeting_location;
    meeting.recorder = payload.recorder;

    Ok((StatusCode::OK, Json(meeting.clone())))
}

pub async fn admin_resolutions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let rows = sqlx::query(
            r#"
            SELECT id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at
            FROM resolutions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(crate::models::ResolutionRecord {
                    id: row.try_get::<uuid::Uuid, _>("id")?.to_string(),
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
                        .try_get::<Option<DateTime<Utc>>, _>("created_at")?
                        .map(|value| value.to_rfc3339()),
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        return Ok((StatusCode::OK, Json(ApiListResponse { items })));
    }

    let resolutions = state.resolutions.read().unwrap();
    let mut items = resolutions.values().cloned().collect::<Vec<_>>();
    items.sort_by(|a, b| {
        a.item_no
            .cmp(&b.item_no)
            .then_with(|| {
                a.created_at
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.created_at.as_deref().unwrap_or_default())
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok((StatusCode::OK, Json(ApiListResponse { items })))
}

pub async fn create_resolution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateResolutionRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    if let Some(pool) = &state.pool {
        let meeting_id = if let Some(id) = payload.meeting_id.clone() {
            id
        } else {
            let fallback = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT id FROM meetings ORDER BY created_at ASC LIMIT 1",
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, "no meetings found"))?;
            fallback.to_string()
        };

        let meeting_uuid = uuid::Uuid::parse_str(&meeting_id).map_err(|_| {
            AppError::new(
                StatusCode::BAD_REQUEST,
                format!("invalid meeting id '{}'", meeting_id),
            )
        })?;

        let agenda_item_uuid = payload
            .agenda_item_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "invalid agenda item id"))?;
        if agenda_item_uuid.is_none() {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "agenda_item_id is required when PostgreSQL is enabled",
            ));
        }
        let resolution_id = uuid::Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO resolutions
                (id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            "#,
        )
        .bind(resolution_id)
        .bind(meeting_uuid)
        .bind(agenda_item_uuid)
        .bind(&payload.item_no)
        .bind(&payload.title)
        .bind(&payload.content)
        .bind(&payload.resolution)
        .bind(&payload.result)
        .execute(pool)
        .await?;

        let resolution = crate::models::ResolutionRecord {
            id: resolution_id.to_string(),
            meeting_id,
            agenda_item_id: payload.agenda_item_id,
            item_no: payload.item_no,
            title: payload.title,
            content: payload.content,
            resolution: payload.resolution,
            result: payload.result,
            created_at: Some(Utc::now().to_rfc3339()),
        };
        return Ok((StatusCode::CREATED, Json(resolution)));
    }

    let meeting_id = state
        .meetings
        .read()
        .unwrap()
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "meeting-001".to_string());

    let resolution = crate::models::ResolutionRecord {
        id: uuid::Uuid::new_v4().to_string(),
        meeting_id,
        agenda_item_id: payload.agenda_item_id,
        item_no: payload.item_no,
        title: payload.title,
        content: payload.content,
        resolution: payload.resolution,
        result: payload.result,
        created_at: None,
    };

    state
        .resolutions
        .write()
        .unwrap()
        .insert(resolution.id.clone(), resolution.clone());
    Ok((StatusCode::CREATED, Json(resolution)))
}

pub async fn admin_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    let documents = state.documents.read().unwrap();
    Ok((
        StatusCode::OK,
        Json(ApiListResponse {
            items: documents.clone(),
        }),
    ))
}

pub async fn upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = crate::services::auth::extract_authenticated_user(&headers, &state).await?;

    let upload_dir = state.config.upload_dir.clone();
    fs::create_dir_all(&upload_dir)
        .with_context(|| format!("failed to create upload directory '{}'", upload_dir))?;

    let mut created: Vec<DocumentRecord> = Vec::new();
    while let Some(field) = multipart.next_field().await? {
        let file_name = field.file_name().unwrap_or("upload.bin").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = field.bytes().await?;
        if bytes.len() > state.config.max_upload_bytes {
            return Err(AppError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "file exceeds maximum upload size of {} bytes",
                    state.config.max_upload_bytes
                ),
            ));
        }

        let safe_name = file_name.rsplit('/').next().unwrap_or(&file_name);
        let storage_path = format!("{}/{}-{}", upload_dir, uuid::Uuid::new_v4(), safe_name);
        fs::write(&storage_path, &bytes)
            .with_context(|| format!("failed to write upload '{}'", storage_path))?;

        let document = DocumentRecord {
            id: uuid::Uuid::new_v4().to_string(),
            file_name: safe_name.to_string(),
            content_type,
            size_bytes: bytes.len(),
            uploaded_by: user.email.clone(),
            uploaded_at: chrono::Utc::now().to_rfc3339(),
            storage_path,
        };

        created.push(document.clone());
        state.documents.write().unwrap().push(document);
    }

    Ok((
        StatusCode::CREATED,
        Json(ApiListResponse { items: created }),
    ))
}

fn parse_date_optional(value: &str) -> Result<Option<NaiveDate>, AppError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map(Some)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "meeting_date must be YYYY-MM-DD"))
}

fn parse_datetime_optional(value: Option<&str>) -> Result<Option<DateTime<Utc>>, AppError> {
    match value {
        None => Ok(None),
        Some(raw) if raw.trim().is_empty() => Ok(None),
        Some(raw) => DateTime::parse_from_rfc3339(raw.trim())
            .map(|value| Some(value.with_timezone(&Utc)))
            .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "datetime must be RFC3339")),
    }
}
