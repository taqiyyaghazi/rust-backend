use crate::application::errors::IdentityApplicationError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

// Valid Argon2id PHC hashes used when an email is not found so timing matches a real verification.
#[cfg(not(target_arch = "wasm32"))]
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

#[cfg(target_arch = "wasm32")]
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=1024,t=1,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn hash_password_with_params(
    raw_password: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<String, IdentityApplicationError> {
    let params = Params::new(m_cost, t_cost, p_cost, None)
        .map_err(|_| IdentityApplicationError::PasswordHashing)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::generate(&mut OsRng);
    argon2
        .hash_password(raw_password, &salt)
        .map(|h| h.to_string())
        .map_err(|_| IdentityApplicationError::PasswordHashing)
}

fn verify_sync(raw_password: &[u8], target_hash: &str, has_user: bool) -> bool {
    let parsed = match PasswordHash::new(target_hash) {
        Ok(parsed) => parsed,
        Err(_) => {
            if has_user {
                tracing::error!("Corrupted password hash in database");
            }
            return false;
        }
    };
    Argon2::default()
        .verify_password(raw_password, &parsed)
        .is_ok()
}

pub async fn hash_password(
    raw_password: String,
    semaphore: &Arc<Semaphore>,
) -> Result<String, IdentityApplicationError> {
    let _permit = semaphore
        .acquire()
        .await
        .map_err(|_| IdentityApplicationError::PasswordHashing)?;

    #[cfg(not(target_arch = "wasm32"))]
    let password_hash = tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(raw_password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| IdentityApplicationError::PasswordHashing)
    })
    .await
    .map_err(|_| IdentityApplicationError::PasswordHashing)??;

    #[cfg(target_arch = "wasm32")]
    let password_hash = hash_password_with_params(raw_password.as_bytes(), 1024, 1, 1)?;

    drop(_permit);
    Ok(password_hash)
}

pub async fn verify_password(
    raw_password: String,
    target_hash: Option<String>,
    semaphore: &Arc<Semaphore>,
) -> Result<bool, IdentityApplicationError> {
    let _permit = semaphore
        .acquire()
        .await
        .map_err(|_| IdentityApplicationError::PasswordHashing)?;

    let has_user = target_hash.is_some();
    let hash_str = target_hash.unwrap_or_else(|| DUMMY_PASSWORD_HASH.to_string());

    #[cfg(not(target_arch = "wasm32"))]
    let is_valid = tokio::task::spawn_blocking(move || {
        verify_sync(raw_password.as_bytes(), &hash_str, has_user)
    })
    .await
    .map_err(|_| IdentityApplicationError::PasswordHashing)?;

    #[cfg(target_arch = "wasm32")]
    let is_valid = verify_sync(raw_password.as_bytes(), &hash_str, has_user);

    drop(_permit);
    Ok(is_valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn hash_and_verify_roundtrip_succeeds() {
        let sem = Arc::new(Semaphore::new(2));
        let hash = hash_password("correct-horse-battery".to_string(), &sem)
            .await
            .expect("hash should succeed");
        assert!(hash.starts_with("$argon2id$"));

        let valid = verify_password(
            "correct-horse-battery".to_string(),
            Some(hash.clone()),
            &sem,
        )
        .await
        .expect("verify should succeed");
        assert!(valid);

        let invalid = verify_password("wrong-password-123".to_string(), Some(hash), &sem)
            .await
            .expect("verify should succeed");
        assert!(!invalid);
    }

    #[tokio::test]
    async fn verify_with_none_runs_dummy_hash_and_returns_false() {
        let sem = Arc::new(Semaphore::new(2));
        let valid = verify_password("any-password-123".to_string(), None, &sem)
            .await
            .expect("dummy verify should not error");
        assert!(!valid);
    }

    #[tokio::test]
    async fn wasm_params_hash_is_accepted_by_verifier() {
        let sem = Arc::new(Semaphore::new(2));
        let hash = hash_password_with_params("edge-worker-pass".as_bytes(), 1024, 1, 1)
            .expect("custom params hash should succeed");
        let valid = verify_password("edge-worker-pass".to_string(), Some(hash), &sem)
            .await
            .expect("verify should succeed");
        assert!(valid);
    }
}
