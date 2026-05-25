#[cfg(test)]
mod tests {
    use crate::journal::sqlite::SqliteJournal;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    async fn make_pool_memory() -> sqlx::SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory)
            .foreign_keys(false);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn integrity_check_ok_on_clean_database() {
        let pool = make_pool_memory().await;
        SqliteJournal::run_migrations(&pool).await.unwrap();
        let j = SqliteJournal::new(pool);
        // Should return Ok(()) on a freshly migrated, uncorrupted database.
        j.integrity_check().await.unwrap();
    }

    #[tokio::test]
    async fn integrity_check_err_on_corrupted_schema() {
        let pool = make_pool_memory().await;
        SqliteJournal::run_migrations(&pool).await.unwrap();

        // Simulate schema corruption by dropping a required table.
        sqlx::query("DROP TABLE journal_entries")
            .execute(&pool)
            .await
            .unwrap();

        let j = SqliteJournal::new(pool);
        // integrity_check is a PRAGMA check — table existence is structural,
        // so the function itself may still return ok; but schema_check catches it.
        let result = j.schema_check().await;
        assert!(
            result.is_err(),
            "schema_check should detect missing journal_entries table"
        );
    }
}
