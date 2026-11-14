use crate::domain::{
    entities::User,
    repositories::{RepositoryError, UserRepository},
    value_objects::{Email, FullName, UserId},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Database persistence model (`users` table row) kept strictly inside `infrastructure`.
/// Prevents SQLx / ORM macros from leaking into the pure Domain Entity (`User`).
#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    full_name: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<UserRow> for User {
    type Error = RepositoryError;

    fn try_from(row: UserRow) -> Result<Self, Self::Error> {
        let email =
            Email::parse(row.email).map_err(|e| RepositoryError::DataCorruption(e.to_string()))?;
        let full_name = FullName::parse(row.full_name)
            .map_err(|e| RepositoryError::DataCorruption(e.to_string()))?;

        Ok(User::reconstitute(
            UserId::from_uuid(row.id),
            email,
            full_name,
            row.is_active,
            row.created_at,
            row.updated_at,
        ))
    }
}

/// PostgreSQL implementation of `UserRepository` powered by `sqlx`.
#[derive(Clone)]
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn save(&self, user: &User) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO users (id, email, full_name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE
            SET email = EXCLUDED.email,
                full_name = EXCLUDED.full_name,
                is_active = EXCLUDED.is_active,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(user.id().as_uuid())
        .bind(user.email().as_str())
        .bind(user.full_name().as_str())
        .bind(user.is_active())
        .bind(user.created_at())
        .bind(user.updated_at())
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, email, full_name, is_active, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        row.map(User::try_from).transpose()
    }
}
