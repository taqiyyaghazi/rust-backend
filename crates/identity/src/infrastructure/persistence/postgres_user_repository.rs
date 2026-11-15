use crate::domain::{
    entities::User,
    repositories::{RepositoryError, UserRepository},
    value_objects::Email,
};
use async_trait::async_trait;
use sqlx::PgPool;

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
}
