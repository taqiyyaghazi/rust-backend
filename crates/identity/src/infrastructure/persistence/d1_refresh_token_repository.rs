use crate::domain::{
    entities::RefreshToken,
    repositories::{RefreshTokenRepository, RepositoryError},
    value_objects::UserId,
};
use crate::infrastructure::persistence::d1_user_repository::{
    is_d1_unique_violation, SendD1Database,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use worker::{send::SendFuture, wasm_bindgen::JsValue, D1Database};

#[derive(Debug, Deserialize)]
struct ReturnedUserIdRow {
    user_id: String,
}

/// Cloudflare D1 (SQLite) implementation of `RefreshTokenRepository`.
#[derive(Clone)]
pub struct D1RefreshTokenRepository {
    db: SendD1Database,
}

impl D1RefreshTokenRepository {
    pub fn new(db: Arc<D1Database>) -> Self {
        Self {
            db: SendD1Database(db),
        }
    }
}

#[async_trait]
impl RefreshTokenRepository for D1RefreshTokenRepository {
    async fn save(&self, token: &RefreshToken) -> Result<(), RepositoryError> {
        let db = self.db.clone();
        let id = token.id().to_string();
        let user_id = token.user_id().as_uuid().to_string();
        let token_hash = token.token_hash().to_string();
        let expires_at = token.expires_at().to_rfc3339();
        let revoked_at = token.revoked_at().map(|dt| dt.to_rfc3339());
        let created_at = token.created_at().to_rfc3339();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, revoked_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            );
            let revoked_js = match revoked_at.as_deref() {
                Some(s) => JsValue::from_str(s),
                None => JsValue::NULL,
            };
            let bound = stmt
                .bind(&[
                    JsValue::from_str(&id),
                    JsValue::from_str(&user_id),
                    JsValue::from_str(&token_hash),
                    JsValue::from_str(&expires_at),
                    revoked_js,
                    JsValue::from_str(&created_at),
                ])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            bound.run().await.map_err(|e| {
                let msg = e.to_string();
                if is_d1_unique_violation(&msg) {
                    RepositoryError::Conflict
                } else {
                    RepositoryError::Storage(msg)
                }
            })?;

            Ok(())
        })
        .await
    }

    async fn revoke_and_get_user_id(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, RepositoryError> {
        let db = self.db.clone();
        let token_hash = token_hash.to_string();
        let now_str = now.to_rfc3339();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "UPDATE refresh_tokens SET revoked_at = ?2 WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2 RETURNING user_id",
            );
            let bound = stmt
                .bind(&[JsValue::from_str(&token_hash), JsValue::from_str(&now_str)])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            let row = bound
                .first::<ReturnedUserIdRow>(None)
                .await
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            match row {
                Some(r) => {
                    let uid = Uuid::parse_str(&r.user_id)
                        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
                    Ok(Some(UserId::from(uid)))
                }
                None => Ok(None),
            }
        })
        .await
    }

    async fn revoke(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepositoryError> {
        let db = self.db.clone();
        let token_hash = token_hash.to_string();
        let now_str = now.to_rfc3339();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "UPDATE refresh_tokens SET revoked_at = ?2 WHERE token_hash = ?1 AND revoked_at IS NULL",
            );
            let bound = stmt
                .bind(&[JsValue::from_str(&token_hash), JsValue::from_str(&now_str)])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            bound
                .run()
                .await
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            Ok(())
        })
        .await
    }
}
