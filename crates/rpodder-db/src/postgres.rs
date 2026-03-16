use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use rpodder_core::error::AppError;
use rpodder_core::repo::{self, Result};
use rpodder_core::types::*;

#[derive(Clone)]
pub struct PgRepo {
    pub pool: PgPool,
}

impl PgRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Internal(e.to_string())
}

// ---------------------------------------------------------------------------
// UserRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    password_hash: String,
    email: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        User {
            id: r.id,
            username: r.username,
            password_hash: r.password_hash,
            email: r.email,
            is_active: r.is_active,
            created_at: r.created_at,
        }
    }
}

impl repo::UserRepo for PgRepo {
    async fn create(&self, username: &str, password_hash: &str, email: Option<&str>) -> Result<User> {
        let id = Uuid::now_v7();
        let row: UserRow = sqlx::query_as(
            "INSERT INTO users (id, username, password_hash, email)
             VALUES ($1, $2, $3, $4)
             RETURNING id, username, password_hash, email, is_active, created_at",
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(dbe) if dbe.is_unique_violation() => {
                AppError::Conflict(format!("user '{username}' already exists"))
            }
            _ => db_err(e),
        })?;
        Ok(row.into())
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE LOWER(username) = LOWER($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }
}

// ---------------------------------------------------------------------------
// SessionRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    token: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Session {
            id: r.id,
            user_id: r.user_id,
            token: r.token,
            expires_at: r.expires_at,
            created_at: r.created_at,
        }
    }
}

impl repo::SessionRepo for PgRepo {
    async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.token)
        .bind(session.expires_at)
        .bind(session.created_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<Session>> {
        let row: Option<SessionRow> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at
             FROM sessions WHERE token = $1 AND expires_at > NOW()",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn delete(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected())
    }
}
