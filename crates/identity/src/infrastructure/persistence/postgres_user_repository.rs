use crate::domain::{
    entities::User,
    repositories::{RepositoryError, UserRepository},
    value_objects::{Email, FullName, UserId},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// PostgreSQL implementation of `UserRepository` powered by `sqlx`.
#[derive(Clone)]
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn map_row(row: sqlx::postgres::PgRow) -> Result<User, RepositoryError> {
        let id: Uuid = row
            .try_get("id")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let email_raw: String = row
            .try_get("email")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let full_name_raw: String = row
            .try_get("full_name")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let password_hash: String = row
            .try_get("password_hash")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let is_active: bool = row
            .try_get("is_active")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let created_at: DateTime<Utc> = row
            .try_get("created_at")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let updated_at: DateTime<Utc> = row
            .try_get("updated_at")
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        let email = Email::parse(email_raw).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let full_name =
            FullName::parse(full_name_raw).map_err(|e| RepositoryError::Storage(e.to_string()))?;

        Ok(User::reconstitute(
            UserId::from(id),
            email,
            full_name,
            password_hash,
            is_active,
            created_at,
            updated_at,
        ))
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn save(&self, user: &User) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO users (id, email, full_name, password_hash, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(user.id().as_uuid())
        .bind(user.email().as_str())
        .bind(user.full_name().as_str())
        .bind(user.password_hash())
        .bind(user.is_active())
        .bind(user.created_at())
        .bind(user.updated_at())
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                RepositoryError::Conflict
            }
            _ => RepositoryError::Storage(e.to_string()),
        })?;

        Ok(())
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool, RepositoryError> {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::Storage(e.to_string()))
    }

    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, full_name, password_hash, is_active, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        row.map(Self::map_row).transpose()
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, full_name, password_hash, is_active, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        row.map(Self::map_row).transpose()
    }
}
