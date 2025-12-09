// [TRADE SECRET] Sales database operations (optional SQLite)
// Persists sales records for auditing and analytics

use crate::error::ApiResult;
use chrono::Utc;

#[cfg(feature = "sqlite")]
pub async fn init_db(pool: &sqlx::SqlitePool) -> ApiResult<()> {
    // Create sales table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sales (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            stripe_session_id TEXT UNIQUE NOT NULL,
            customer_email TEXT NOT NULL,
            tier TEXT NOT NULL,
            amount_cents INTEGER NOT NULL,
            license_key TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(feature = "sqlite")]
pub async fn save_sale(
    pool: &sqlx::SqlitePool,
    stripe_session_id: &str,
    customer_email: &str,
    tier: &str,
    amount_cents: i64,
    license_key: &str,
) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO sales (stripe_session_id, customer_email, tier, amount_cents, license_key, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(stripe_session_id)
    .bind(customer_email)
    .bind(tier)
    .bind(amount_cents)
    .bind(license_key)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(feature = "sqlite")]
pub async fn get_sales_count(pool: &sqlx::SqlitePool, tier: &str) -> ApiResult<i64> {
    let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sales WHERE tier = ?")
        .bind(tier)
        .fetch_one(pool)
        .await?;

    Ok(result.0)
}

// Stubs for when sqlite feature is disabled
#[cfg(not(feature = "sqlite"))]
pub async fn init_db(_pool: &()) -> ApiResult<()> {
    Ok(())
}

#[cfg(not(feature = "sqlite"))]
pub async fn save_sale(
    _pool: &(),
    _stripe_session_id: &str,
    _customer_email: &str,
    _tier: &str,
    _amount_cents: i64,
    _license_key: &str,
) -> ApiResult<()> {
    Ok(())
}

#[cfg(not(feature = "sqlite"))]
pub async fn get_sales_count(_pool: &(), _tier: &str) -> ApiResult<i64> {
    Ok(0)
}
