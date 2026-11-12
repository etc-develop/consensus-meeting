use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    #[serde(skip_serializing)]
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootstrapRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: String,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub uploaded_by: String,
    pub uploaded_at: String,
    pub storage_path: String,
}
