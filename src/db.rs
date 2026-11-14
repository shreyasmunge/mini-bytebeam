//! Create a Postgres connection pool and run SQL migrations.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool, String> {
    PgPoolOptions::new()
        .max_connections(max_connections.max(5))
        .connect(database_url)
        .await
        .map_err(|e| format!("connect failed: {e}"))
}

/// Runs every `.sql` file in `./migrations` in name order.
pub async fn migrate(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| format!("migrate failed: {e}"))
}
