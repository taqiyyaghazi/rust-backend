use crate::domain::{
    entities::User,
    repositories::{RepositoryError, UserRepository},
    value_objects::{Email, FullName, UserId},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use worker::{send::SendFuture, wasm_bindgen::JsValue, D1Database};

/// Wrapper allowing `Arc<D1Database>` to satisfy `Send + Sync` bounds on `wasm32`
/// where the Cloudflare Workers runtime is strictly single-threaded.
#[derive(Clone)]
pub(crate) struct SendD1Database(pub Arc<D1Database>);

// SAFETY: Cloudflare Workers (`wasm32-unknown-unknown`) runs in a single-threaded isolate.
unsafe impl Send for SendD1Database {}
// SAFETY: Cloudflare Workers (`wasm32-unknown-unknown`) runs in a single-threaded isolate.
unsafe impl Sync for SendD1Database {}

pub(crate) fn is_d1_unique_violation(msg: &str) -> bool {
    msg.contains("UNIQUE constraint failed")
}

fn deserialize_sqlite_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SqliteBool {
        Bool(bool),
        Int(i64),
    }

    match SqliteBool::deserialize(deserializer)? {
        SqliteBool::Bool(b) => Ok(b),
        SqliteBool::Int(i) => Ok(i != 0),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct D1UserRow {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub password_hash: String,
    #[serde(deserialize_with = "deserialize_sqlite_bool")]
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl D1UserRow {
    pub(crate) fn into_domain(self) -> Result<User, RepositoryError> {
        let id = Uuid::parse_str(&self.id).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let email =
            Email::parse(self.email).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let full_name =
            FullName::parse(self.full_name).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let updated_at = DateTime::parse_from_rfc3339(&self.updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;

        Ok(User::reconstitute(
            UserId::from(id),
            email,
            full_name,
            self.password_hash,
            self.is_active,
            created_at,
            updated_at,
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ExistsRow {
    #[allow(dead_code)]
    found: i64,
}

/// Cloudflare D1 (SQLite) implementation of `UserRepository`.
#[derive(Clone)]
pub struct D1UserRepository {
    db: SendD1Database,
}

impl D1UserRepository {
    pub fn new(db: Arc<D1Database>) -> Self {
        Self {
            db: SendD1Database(db),
        }
    }
}

#[async_trait]
impl UserRepository for D1UserRepository {
    async fn save(&self, user: &User) -> Result<(), RepositoryError> {
        let db = self.db.clone();
        let id = user.id().as_uuid().to_string();
        let email = user.email().as_str().to_string();
        let full_name = user.full_name().as_str().to_string();
        let password_hash = user.password_hash().to_string();
        let is_active = if user.is_active() { 1.0 } else { 0.0 };
        let created_at = user.created_at().to_rfc3339();
        let updated_at = user.updated_at().to_rfc3339();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "INSERT INTO users (id, email, full_name, password_hash, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            );
            let bound = stmt
                .bind(&[
                    JsValue::from_str(&id),
                    JsValue::from_str(&email),
                    JsValue::from_str(&full_name),
                    JsValue::from_str(&password_hash),
                    JsValue::from_f64(is_active),
                    JsValue::from_str(&created_at),
                    JsValue::from_str(&updated_at),
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

    async fn exists_by_email(&self, email: &Email) -> Result<bool, RepositoryError> {
        let db = self.db.clone();
        let email_str = email.as_str().to_string();

        SendFuture::new(async move {
            let stmt = db
                .0
                .prepare("SELECT 1 as found FROM users WHERE email = ?1 LIMIT 1");
            let bound = stmt
                .bind(&[JsValue::from_str(&email_str)])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            let row = bound
                .first::<ExistsRow>(None)
                .await
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            Ok(row.is_some())
        })
        .await
    }

    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
        let db = self.db.clone();
        let email_str = email.as_str().to_string();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "SELECT id, email, full_name, password_hash, is_active, created_at, updated_at FROM users WHERE email = ?1 LIMIT 1",
            );
            let bound = stmt
                .bind(&[JsValue::from_str(&email_str)])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            let row = bound
                .first::<D1UserRow>(None)
                .await
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            row.map(D1UserRow::into_domain).transpose()
        })
        .await
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let db = self.db.clone();
        let id_str = id.as_uuid().to_string();

        SendFuture::new(async move {
            let stmt = db.0.prepare(
                "SELECT id, email, full_name, password_hash, is_active, created_at, updated_at FROM users WHERE id = ?1 LIMIT 1",
            );
            let bound = stmt
                .bind(&[JsValue::from_str(&id_str)])
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            let row = bound
                .first::<D1UserRow>(None)
                .await
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;

            row.map(D1UserRow::into_domain).transpose()
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn d1_user_row_converts_sqlite_integer_bool_and_rfc3339_timestamps() {
        let uid = Uuid::new_v4();
        let json = serde_json::json!({
            "id": uid.to_string(),
            "email": "d1user@example.com",
            "full_name": "D1 Edge User",
            "password_hash": "$argon2id$v=19$m=1024,t=1,p=1$c2FsdHNhbHQ$aGFzaGhhc2g",
            "is_active": 1,
            "created_at": "2026-11-15T12:00:00Z",
            "updated_at": "2026-11-15T12:00:00Z"
        });
        let row: D1UserRow = serde_json::from_value(json).expect("deserialize D1UserRow");
        let user = row.into_domain().expect("convert to domain User");
        assert_eq!(user.id().as_uuid(), uid);
        assert_eq!(user.email().as_str(), "d1user@example.com");
        assert_eq!(user.full_name().as_str(), "D1 Edge User");
        assert!(user.is_active());
    }

    #[test]
    fn d1_user_row_rejects_invalid_rfc3339_timestamp() {
        let json = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "email": "d1user@example.com",
            "full_name": "D1 Edge User",
            "password_hash": "hash",
            "is_active": 0,
            "created_at": "not-a-timestamp",
            "updated_at": "2026-11-15T12:00:00Z"
        });
        let row: D1UserRow = serde_json::from_value(json).expect("deserialize D1UserRow");
        assert!(matches!(row.into_domain(), Err(RepositoryError::Storage(_))));
    }

    #[test]
    fn detects_d1_sqlite_unique_constraint_errors() {
        assert!(is_d1_unique_violation(
            "D1_ERROR: UNIQUE constraint failed: users.email: SQLITE_CONSTRAINT"
        ));
        assert!(!is_d1_unique_violation("D1_ERROR: syntax error"));
    }
}
