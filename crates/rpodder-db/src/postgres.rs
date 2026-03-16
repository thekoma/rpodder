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

// ---------------------------------------------------------------------------
// DeviceRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    user_id: Uuid,
    device_id: String,
    caption: String,
    device_type: String,
    sync_group_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DeviceRow> for Device {
    fn from(r: DeviceRow) -> Self {
        Device {
            id: r.id,
            user_id: r.user_id,
            device_id: r.device_id,
            caption: r.caption,
            device_type: parse_device_type(&r.device_type),
            sync_group_id: r.sync_group_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

fn parse_device_type(s: &str) -> DeviceType {
    match s {
        "desktop" => DeviceType::Desktop,
        "laptop" => DeviceType::Laptop,
        "mobile" => DeviceType::Mobile,
        "server" => DeviceType::Server,
        "tablet" => DeviceType::Tablet,
        _ => DeviceType::Other,
    }
}

fn device_type_str(dt: DeviceType) -> &'static str {
    match dt {
        DeviceType::Desktop => "desktop",
        DeviceType::Laptop => "laptop",
        DeviceType::Mobile => "mobile",
        DeviceType::Server => "server",
        DeviceType::Tablet => "tablet",
        DeviceType::Other => "other",
    }
}

impl repo::DeviceRepo for PgRepo {
    async fn upsert(&self, device: &Device) -> Result<Device> {
        let row: DeviceRow = sqlx::query_as(
            "INSERT INTO devices (id, user_id, device_id, caption, device_type, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (user_id, device_id)
             DO UPDATE SET caption = EXCLUDED.caption, device_type = EXCLUDED.device_type, updated_at = EXCLUDED.updated_at
             RETURNING id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at",
        )
        .bind(device.id)
        .bind(device.user_id)
        .bind(&device.device_id)
        .bind(&device.caption)
        .bind(device_type_str(device.device_type))
        .bind(device.created_at)
        .bind(device.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.into())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Device>> {
        let rows: Vec<DeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_uid(&self, user_id: Uuid, device_id: &str) -> Result<Option<Device>> {
        let row: Option<DeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
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
