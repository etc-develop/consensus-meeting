use anyhow::{Context, Result, bail};
use consensus_meeting::config::AppConfig;
use sqlx::{Executor, postgres::PgPoolOptions};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();
    let database_url = config
        .database_url
        .ok_or_else(|| anyhow::anyhow!("DATABASE_URL is not set"))?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .with_context(|| "failed to connect to database using DATABASE_URL")?;

    println!("[reset-db] dropping and recreating public schema...");
    pool.execute("DROP SCHEMA IF EXISTS public CASCADE;").await?;
    pool.execute("CREATE SCHEMA public;").await?;

    println!("[reset-db] applying migrations...");
    let migrations = sqlx::migrate!("./src/databases/migrations");
    migrations.run(&pool).await?;

    let applied_count = migrations.iter().count();
    if applied_count == 0 {
        bail!("no migration files found in ./src/databases/migrations");
    }

    println!("[reset-db] done. applied {} migration(s).", applied_count);
    Ok(())
}
