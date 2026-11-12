pub mod api;
pub mod app;
pub mod config;
pub mod databases;
pub mod models;
pub mod routers;
pub mod services;

use crate::app::{build_router, create_app_state};
use crate::databases::seeds::seed;
use std::net::SocketAddr;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Copy)]
struct Utc8Timer;

impl FormatTime for Utc8Timer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let offset =
            chrono::FixedOffset::east_opt(8 * 3600).expect("UTC+8 fixed offset must be valid");
        write!(
            w,
            "{}",
            chrono::Utc::now()
                .with_timezone(&offset)
                .format("%Y-%m-%d %H:%M:%S")
        )
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer().with_timer(Utc8Timer))
        .init();

    let state = create_app_state().await?;
    seed::seed_demo_data(&state).await?;

    let bind_addr: String = state.config.bind_addr.clone();
    let app = build_router(state);
    let bind_addr: SocketAddr = bind_addr
        .parse()
        .map_err(|err| anyhow::anyhow!("APP_BIND_ADDR must be a valid socket address: {err}"))?;

    tracing::info!(bind_addr = %bind_addr, "Starting API");
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
