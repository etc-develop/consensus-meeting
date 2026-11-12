use anyhow::{Context, Result};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sqlx::Row;

use crate::{
    app::AppState,
    config::AppConfig,
    models::{JwtClaims, UserRecord},
};

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn create_access_token(user: &UserRecord, config: &AppConfig) -> Result<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now() + Duration::minutes(30)).timestamp() as usize;
    let claims = JwtClaims {
        sub: user.id.clone(),
        email: user.email.clone(),
        exp,
        iat: now,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn create_refresh_token(user: &UserRecord, config: &AppConfig) -> Result<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now() + Duration::days(14)).timestamp() as usize;
    let claims = JwtClaims {
        sub: user.id.clone(),
        email: user.email.clone(),
        exp,
        iat: now,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn decode_token(token: &str, config: &AppConfig) -> Result<JwtClaims> {
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(token_data.claims)
}

pub async fn extract_authenticated_user(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<UserRecord> {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .context("missing authorization header")?;

    let token = authorization
        .strip_prefix("Bearer ")
        .context("authorization header must use bearer token")?;
    let claims = decode_token(token, &state.config)?;

    if let Some(pool) = &state.pool {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, is_active, created_at
            FROM users
            WHERE id::text = $1 OR lower(email) = lower($2)
            LIMIT 1
            "#,
        )
        .bind(&claims.sub)
        .bind(&claims.email)
        .fetch_optional(pool)
        .await?
        .context("user not found")?;

        let id: uuid::Uuid = row.try_get("id")?;
        let email: String = row.try_get("email")?;
        let display_name: String = row.try_get("display_name")?;
        let password_hash: String = row.try_get("password_hash")?;
        let is_active: bool = row.try_get("is_active")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;

        return Ok(UserRecord {
            id: id.to_string(),
            email,
            display_name,
            password_hash,
            is_active,
            created_at: created_at.to_rfc3339(),
        });
    }

    let users = state.users.read().unwrap();
    let user = users
        .values()
        .find(|value| value.id == claims.sub || value.email == claims.email)
        .cloned()
        .context("user not found")?;

    Ok(user)
}
