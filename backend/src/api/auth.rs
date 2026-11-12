use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use sqlx::Row;

use crate::{
    api::AppError,
    app::AppState,
    models::{BootstrapRequest, LoginRequest, TokenResponse},
    services::auth::{
        create_access_token, create_refresh_token, extract_authenticated_user, hash_password,
        verify_password,
    },
};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, is_active, created_at
            FROM users
            WHERE lower(email) = lower($1)
            LIMIT 1
            "#,
        )
        .bind(&payload.email)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;

        let user = crate::models::UserRecord {
            id: row.try_get::<uuid::Uuid, _>("id")?.to_string(),
            email: row.try_get("email")?,
            display_name: row.try_get("display_name")?,
            password_hash: row.try_get("password_hash")?,
            is_active: row.try_get("is_active")?,
            created_at: row
                .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")?
                .to_rfc3339(),
        };

        if !verify_password(&payload.password, &user.password_hash)? {
            return Err(AppError::new(
                StatusCode::UNAUTHORIZED,
                "invalid credentials",
            ));
        }

        let access_token = create_access_token(&user, &state.config)?;
        let refresh_token = create_refresh_token(&user, &state.config)?;
        let response = TokenResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in_seconds: 1800,
        };
        return Ok((StatusCode::OK, Json(response)));
    }

    let users = state.users.read().unwrap();
    let user = users
        .values()
        .find(|value| value.email.eq_ignore_ascii_case(&payload.email))
        .cloned()
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;

    if !verify_password(&payload.password, &user.password_hash)? {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }

    let access_token = create_access_token(&user, &state.config)?;
    let refresh_token = create_refresh_token(&user, &state.config)?;
    let response = TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in_seconds: 1800,
    };
    Ok((StatusCode::OK, Json(response)))
}

pub async fn bootstrap_admin(
    State(state): State<AppState>,
    Json(payload): Json<BootstrapRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(pool) = &state.pool {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;
        if count > 0 {
            return Err(AppError::new(
                StatusCode::CONFLICT,
                "admin bootstrap is only allowed when no users exist",
            ));
        }

        let password_hash = hash_password(&payload.password)?;
        let id = uuid::Uuid::new_v4();
        let created_at = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, display_name, password_hash, is_active, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id)
        .bind(&payload.email)
        .bind(&payload.display_name)
        .bind(&password_hash)
        .bind(true)
        .bind(created_at)
        .execute(pool)
        .await?;

        let user = crate::models::UserRecord {
            id: id.to_string(),
            email: payload.email.clone(),
            display_name: payload.display_name.clone(),
            password_hash,
            is_active: true,
            created_at: created_at.to_rfc3339(),
        };

        let access_token = create_access_token(&user, &state.config)?;
        let refresh_token = create_refresh_token(&user, &state.config)?;
        return Ok((
            StatusCode::CREATED,
            Json(TokenResponse {
                access_token,
                refresh_token,
                token_type: "Bearer".to_string(),
                expires_in_seconds: 1800,
            }),
        ));
    }

    let mut users = state.users.write().unwrap();
    if !users.is_empty() {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "admin bootstrap is only allowed when no users exist",
        ));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = crate::models::UserRecord {
        id: uuid::Uuid::new_v4().to_string(),
        email: payload.email.clone(),
        display_name: payload.display_name.clone(),
        password_hash,
        is_active: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    users.insert(payload.email.to_ascii_lowercase(), user.clone());

    let access_token = create_access_token(&user, &state.config)?;
    let refresh_token = create_refresh_token(&user, &state.config)?;
    Ok((
        StatusCode::CREATED,
        Json(TokenResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in_seconds: 1800,
        }),
    ))
}

pub async fn current_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user = extract_authenticated_user(&headers, &state).await?;
    Ok((StatusCode::OK, Json(user)))
}
