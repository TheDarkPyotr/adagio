#[cfg(test)]
mod tests {
    use crate::journal::sqlite::SqliteJournal;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;
    use tempfile::TempDir;

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

    // T011 — migration 004 (E2EE tables) runs cleanly on a fresh database.
    #[tokio::test]
    async fn migration_004_e2ee_tables_created() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&format!("sqlite://{}?mode=rwc", db_path.display()))
                    .unwrap()
                    .journal_mode(SqliteJournalMode::Wal)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        SqliteJournal::run_migrations(&pool).await.unwrap();
        // Both E2EE tables must exist.
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('e2ee_account_keys','e2ee_folder_state')"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            count.0, 2,
            "both E2EE tables must be created by migration 004"
        );
    }
}
