//! Create a Postgres connection pool and run SQL migrations.
//!
//! A "pool" = several open DB connections we reuse.
//! That is faster than opening a new connection per request.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool, String> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|e| format!("connect failed: {e}"))
}

/// Runs every `.sql` file in `./migrations` in name order.
/// Safe to call on every startup (sqlx tracks what already ran).
pub async fn migrate(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| format!("migrate failed: {e}"))
}
