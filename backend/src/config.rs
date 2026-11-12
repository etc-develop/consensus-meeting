const DEFAULT_CORS_ORIGINS: &str =
    "http://localhost:5173,http://127.0.0.1:5173,http://localhost:4173,http://127.0.0.1:4173";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub jwt_secret: String,
    pub database_url: Option<String>,
    pub upload_dir: String,
    pub max_upload_bytes: usize,
    pub cors_allowed_origins: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let bind_addr =
            std::env::var("APP_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "development-secret-change-me-please-32char".to_string());
        let database_url = std::env::var("DATABASE_URL").ok();
        let upload_dir =
            std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage/uploads".to_string());
        let max_upload_bytes = std::env::var("MAX_UPLOAD_BYTES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(26_214_400);
        let cors_allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| DEFAULT_CORS_ORIGINS.to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        Self {
            bind_addr,
            jwt_secret,
            database_url,
            upload_dir,
            max_upload_bytes,
            cors_allowed_origins,
        }
    }
}
