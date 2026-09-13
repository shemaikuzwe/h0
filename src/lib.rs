pub mod commands;
pub mod models;
pub mod schema;

use anyhow::Context;
use diesel::pg::PgConnection;
use diesel::prelude::*;

pub fn establish_connection() -> anyhow::Result<PgConnection> {
    let _ = dotenvy::dotenv();
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    PgConnection::establish(&database_url)
        .with_context(|| format!("failed to connect to {database_url}"))
}
