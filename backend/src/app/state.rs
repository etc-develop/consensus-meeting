use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    config::AppConfig,
    models::{
        AgendaItemRecord, DocumentRecord, MeetingPromotionRecord, MeetingRecord, ResolutionRecord,
        UserRecord,
    },
};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub users: Arc<RwLock<HashMap<String, UserRecord>>>,
    pub meetings: Arc<RwLock<HashMap<String, MeetingRecord>>>,
    pub meeting_promotions: Arc<RwLock<HashMap<String, MeetingPromotionRecord>>>,
    pub agenda_items: Arc<RwLock<HashMap<String, AgendaItemRecord>>>,
    pub resolutions: Arc<RwLock<HashMap<String, ResolutionRecord>>>,
    pub documents: Arc<RwLock<Vec<DocumentRecord>>>,
    pub pool: Option<PgPool>,
}

pub async fn create_app_state() -> Result<AppState> {
    let config = Arc::new(AppConfig::from_env());
    let state = AppState {
        config: config.clone(),
        users: Arc::new(RwLock::new(HashMap::new())),
        meetings: Arc::new(RwLock::new(HashMap::new())),
        meeting_promotions: Arc::new(RwLock::new(HashMap::new())),
        agenda_items: Arc::new(RwLock::new(HashMap::new())),
        resolutions: Arc::new(RwLock::new(HashMap::new())),
        documents: Arc::new(RwLock::new(Vec::new())),
        pool: None,
    };

    if let Some(database_url) = config.database_url.clone() {
        match PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                let pool = pool.clone();
                let migrations = sqlx::migrate!("./src/databases/migrations");
                if let Err(error) = migrations.run(&pool).await {
                    tracing::warn!(
                        ?error,
                        "migration failed; continuing with in-memory fallback"
                    );
                }
                let mut next_state = state.clone();
                next_state.pool = Some(pool);
                return Ok(next_state);
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "database unavailable; continuing with in-memory fallback"
                );
            }
        }
    }

    Ok(state)
}
