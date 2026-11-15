use crate::domain::{
    entities::RefreshToken,
    repositories::{RefreshTokenRepository, RepositoryError},
    value_objects::UserId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresRefreshTokenRepository {
    pool: PgPool,
}

impl PostgresRefreshTokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RefreshTokenRepository for PostgresRefreshTokenRepository {
    async fn save(&self, token: &RefreshToken) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, revoked_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(token.id())
        .bind(token.user_id().as_uuid())
        .bind(token.token_hash())
        .bind(token.expires_at())
        .bind(token.revoked_at())
        .bind(token.created_at())
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

    async fn revoke_and_get_user_id(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, RepositoryError> {
        let user_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = $2
            WHERE token_hash = $1
              AND revoked_at IS NULL
              AND expires_at > $2
            RETURNING user_id
            "#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        Ok(user_id.map(UserId::from))
    }

    async fn revoke(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = $2
            WHERE token_hash = $1
              AND revoked_at IS NULL
            "#,
        )
        .bind(token_hash)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        Ok(())
    }
}
