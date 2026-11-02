//! The `rpodder` binary must carry its own schema: release binaries ship
//! without the repo's `migrations/` directory (issue #36).

use std::process::Command;

use rpodder_db::Db;

#[tokio::test]
async fn migrate_works_from_a_directory_without_migration_files() {
    let dir = std::env::temp_dir().join(format!("rpodder-migrate-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&dir).unwrap();
    let db_url = format!("sqlite://{}", dir.join("rpodder.db").display());

    let output = Command::new(env!("CARGO_BIN_EXE_rpodder"))
        .arg("migrate")
        .current_dir(&dir)
        .env_clear()
        .env("RPODDER_DATABASE_URL", &db_url)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "`rpodder migrate` failed outside the repo:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A clean exit is not enough: the first and the last migration both landed.
    let Db::Sqlite(pool) = Db::connect(&db_url).await.unwrap() else {
        panic!("expected SQLite");
    };
    let found: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('users', 'idx_epactions_user_created')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    pool.close().await;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(found, 2, "schema missing after `rpodder migrate`");
}
