use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use rpodder_core::error::AppError;
use rpodder_core::repo::{self, Result};
use rpodder_core::types::*;

#[derive(Clone)]
pub struct SqliteRepo {
    pub pool: SqlitePool,
}

impl SqliteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn uuid_str(u: &Uuid) -> String {
    u.to_string()
}

// ---------------------------------------------------------------------------
// UserRepo
// ---------------------------------------------------------------------------

impl repo::UserRepo for SqliteRepo {
    async fn create(&self, username: &str, password_hash: &str, email: Option<&str>) -> Result<User> {
        let id = Uuid::now_v7();
        let id_s = uuid_str(&id);
        let now = Utc::now();
        let now_s = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, email, is_active, created_at)
             VALUES (?, ?, ?, ?, 1, ?)",
        )
        .bind(&id_s)
        .bind(username)
        .bind(password_hash)
        .bind(email)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(dbe) if dbe.message().contains("UNIQUE") => {
                AppError::Conflict(format!("user '{username}' already exists"))
            }
            _ => AppError::Internal(e.to_string()),
        })?;

        Ok(User {
            id,
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            email: email.map(|e| e.to_string()),
            is_active: true,
            created_at: now,
        })
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE username = ? COLLATE NOCASE",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE id = ?",
        )
        .bind(uuid_str(&id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }
}

#[derive(sqlx::FromRow)]
struct SqliteUserRow {
    id: String,
    username: String,
    password_hash: String,
    email: Option<String>,
    is_active: bool,
    created_at: String,
}

impl From<SqliteUserRow> for User {
    fn from(r: SqliteUserRow) -> Self {
        User {
            id: r.id.parse().unwrap_or_default(),
            username: r.username,
            password_hash: r.password_hash,
            email: r.email,
            is_active: r.is_active,
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// SessionRepo
// ---------------------------------------------------------------------------

impl repo::SessionRepo for SqliteRepo {
    async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(&session.id))
        .bind(uuid_str(&session.user_id))
        .bind(&session.token)
        .bind(session.expires_at.to_rfc3339())
        .bind(session.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<Session>> {
        let now = Utc::now().to_rfc3339();
        let row: Option<SqliteSessionRow> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at
             FROM sessions WHERE token = ? AND expires_at > ?",
        )
        .bind(token)
        .bind(&now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn delete(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[derive(sqlx::FromRow)]
struct SqliteSessionRow {
    id: String,
    user_id: String,
    token: String,
    expires_at: String,
    created_at: String,
}

impl From<SqliteSessionRow> for Session {
    fn from(r: SqliteSessionRow) -> Self {
        Session {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            token: r.token,
            expires_at: r.expires_at.parse().unwrap_or_default(),
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rpodder_core::repo::{SessionRepo, UserRepo};

    async fn setup() -> SqliteRepo {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let schema = std::fs::read_to_string("../../migrations/sqlite/001_initial.up.sql").unwrap();
        sqlx::raw_sql(&schema).execute(&pool).await.unwrap();
        SqliteRepo::new(pool)
    }

    // === UserRepo tests ===

    #[tokio::test]
    async fn create_user_and_find_by_username() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "Alice", "hash123", Some("alice@example.com")).await.unwrap();

        assert_eq!(user.username, "Alice");
        assert_eq!(user.email.as_deref(), Some("alice@example.com"));
        assert!(user.is_active);

        let found = repo.find_by_username("Alice").await.unwrap().unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.password_hash, "hash123");
    }

    #[tokio::test]
    async fn find_by_username_case_insensitive() {
        let repo = setup().await;
        UserRepo::create(&repo, "Bob", "hash", None).await.unwrap();

        let found = repo.find_by_username("bob").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "Bob");

        let found = repo.find_by_username("BOB").await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn find_by_username_not_found() {
        let repo = setup().await;
        let found = repo.find_by_username("nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_id() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "Charlie", "hash", None).await.unwrap();

        let found = repo.find_by_id(user.id).await.unwrap().unwrap();
        assert_eq!(found.username, "Charlie");
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let repo = setup().await;
        let found = repo.find_by_id(Uuid::now_v7()).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn create_duplicate_username_fails() {
        let repo = setup().await;
        UserRepo::create(&repo, "Dave", "hash1", None).await.unwrap();

        let result = UserRepo::create(&repo, "Dave", "hash2", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Conflict(msg) => assert!(msg.contains("Dave")),
            other => panic!("expected Conflict, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_user_without_email() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "NoEmail", "hash", None).await.unwrap();
        assert!(user.email.is_none());

        let found = repo.find_by_username("NoEmail").await.unwrap().unwrap();
        assert!(found.email.is_none());
    }

    // === SessionRepo tests ===

    #[tokio::test]
    async fn create_session_and_find_by_token() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "SessionUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "test-token-abc123".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        let found = repo.find_by_token("test-token-abc123").await.unwrap().unwrap();
        assert_eq!(found.user_id, user.id);
        assert_eq!(found.token, "test-token-abc123");
    }

    #[tokio::test]
    async fn find_by_token_not_found() {
        let repo = setup().await;
        let found = repo.find_by_token("nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_token_expired_returns_none() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "ExpiredUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "expired-token".to_string(),
            expires_at: Utc::now() - Duration::hours(1), // already expired
            created_at: Utc::now() - Duration::hours(2),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        let found = repo.find_by_token("expired-token").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn delete_session() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "DeleteUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "delete-me".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        // Confirm it exists
        assert!(repo.find_by_token("delete-me").await.unwrap().is_some());

        // Delete it
        SessionRepo::delete(&repo, "delete-me").await.unwrap();

        // Confirm it's gone
        assert!(repo.find_by_token("delete-me").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_expired_sessions() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "CleanupUser", "hash", None).await.unwrap();

        // Create 2 expired sessions and 1 valid
        for (i, hours_offset) in [(-2i64), (-1), 1].iter().enumerate() {
            let session = Session {
                id: Uuid::now_v7(),
                user_id: user.id,
                token: format!("token-{i}"),
                expires_at: Utc::now() + Duration::hours(*hours_offset),
                created_at: Utc::now() - Duration::hours(3),
            };
            SessionRepo::create(&repo, &session).await.unwrap();
        }

        let deleted = repo.delete_expired().await.unwrap();
        assert_eq!(deleted, 2);

        // The valid one should still exist
        assert!(repo.find_by_token("token-2").await.unwrap().is_some());
        // The expired ones should be gone
        assert!(repo.find_by_token("token-0").await.unwrap().is_none());
        assert!(repo.find_by_token("token-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_session_is_ok() {
        let repo = setup().await;
        // Should not error
        SessionRepo::delete(&repo, "does-not-exist").await.unwrap();
    }

    #[tokio::test]
    async fn multiple_sessions_per_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "MultiSession", "hash", None).await.unwrap();

        for i in 0..3 {
            let session = Session {
                id: Uuid::now_v7(),
                user_id: user.id,
                token: format!("multi-token-{i}"),
                expires_at: Utc::now() + Duration::hours(1),
                created_at: Utc::now(),
            };
            SessionRepo::create(&repo, &session).await.unwrap();
        }

        // All three should be findable
        for i in 0..3 {
            let found = repo.find_by_token(&format!("multi-token-{i}")).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().user_id, user.id);
        }
    }
}
