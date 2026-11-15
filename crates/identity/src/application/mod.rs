pub mod commands;
pub mod dto;
pub mod errors;
pub mod queries;
pub mod services;

pub use dto::{AuthResponseDto, TokenPairDto, UserDto};
pub use errors::IdentityApplicationError;

#[cfg(test)]
mod tests {
    use super::{
        commands::{
            LoginUserCommand, LoginUserHandler, LogoutUserCommand, LogoutUserHandler,
            RefreshTokenCommand, RefreshTokenHandler, RegisterUserCommand, RegisterUserHandler,
        },
        queries::{GetCurrentUserHandler, GetCurrentUserQuery},
        services::TokenService,
        IdentityApplicationError,
    };
    use crate::domain::{
        entities::{RefreshToken, User},
        repositories::{RefreshTokenRepository, RepositoryError, UserRepository},
        value_objects::{Email, FullName, UserId},
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    use shared_kernel::presentation::{JwtAuthConfig, JwtClaims};
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct InMemoryUserRepo {
        users: Mutex<HashMap<UserId, User>>,
    }

    #[async_trait]
    impl UserRepository for InMemoryUserRepo {
        async fn save(&self, user: &User) -> Result<(), RepositoryError> {
            let mut guard = self.users.lock().await;
            if guard
                .values()
                .any(|u| u.email().as_str() == user.email().as_str())
            {
                return Err(RepositoryError::Conflict);
            }
            guard.insert(user.id(), user.clone());
            Ok(())
        }

        async fn exists_by_email(&self, email: &Email) -> Result<bool, RepositoryError> {
            let guard = self.users.lock().await;
            Ok(guard.values().any(|u| u.email().as_str() == email.as_str()))
        }

        async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
            let guard = self.users.lock().await;
            Ok(guard
                .values()
                .find(|u| u.email().as_str() == email.as_str())
                .cloned())
        }

        async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
            let guard = self.users.lock().await;
            Ok(guard.get(&id).cloned())
        }
    }

    #[derive(Clone)]
    struct StoredToken {
        user_id: UserId,
        expires_at: DateTime<Utc>,
        revoked_at: Option<DateTime<Utc>>,
    }

    #[derive(Default)]
    struct InMemoryRefreshTokenRepo {
        tokens: Mutex<HashMap<String, StoredToken>>,
    }

    #[async_trait]
    impl RefreshTokenRepository for InMemoryRefreshTokenRepo {
        async fn save(&self, token: &RefreshToken) -> Result<(), RepositoryError> {
            let mut guard = self.tokens.lock().await;
            guard.insert(
                token.token_hash().to_string(),
                StoredToken {
                    user_id: token.user_id(),
                    expires_at: token.expires_at(),
                    revoked_at: token.revoked_at(),
                },
            );
            Ok(())
        }

        async fn revoke_and_get_user_id(
            &self,
            token_hash: &str,
            now: DateTime<Utc>,
        ) -> Result<Option<UserId>, RepositoryError> {
            let mut guard = self.tokens.lock().await;
            let Some(entry) = guard.get_mut(token_hash) else {
                return Ok(None);
            };
            if entry.revoked_at.is_some() || entry.expires_at <= now {
                return Ok(None);
            }
            entry.revoked_at = Some(now);
            Ok(Some(entry.user_id))
        }

        async fn revoke(
            &self,
            token_hash: &str,
            now: DateTime<Utc>,
        ) -> Result<(), RepositoryError> {
            let mut guard = self.tokens.lock().await;
            if let Some(entry) = guard.get_mut(token_hash) {
                if entry.revoked_at.is_none() {
                    entry.revoked_at = Some(now);
                }
            }
            Ok(())
        }
    }

    fn test_jwt_config() -> JwtAuthConfig {
        JwtAuthConfig::new("test-secret-key-at-least-32-bytes-long", 900)
    }

    fn test_semaphore() -> Arc<tokio::sync::Semaphore> {
        Arc::new(tokio::sync::Semaphore::new(4))
    }

    #[tokio::test]
    async fn register_issues_token_pair_and_stores_hashed_refresh_token() {
        let user_repo = Arc::new(InMemoryUserRepo::default());
        let token_repo = Arc::new(InMemoryRefreshTokenRepo::default());
        let jwt_config = test_jwt_config();
        let token_service = Arc::new(TokenService::new(token_repo.clone(), jwt_config.clone(), 7));
        let handler = RegisterUserHandler::new(user_repo, token_service, test_semaphore());

        let res = handler
            .handle(RegisterUserCommand {
                email: "budi@example.com".to_string(),
                full_name: "Budi Santoso".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(res.user.email, "budi@example.com");
        assert_eq!(res.expires_in, 900);
        assert!(!res.refresh_token.is_empty());

        let decoded = decode::<JwtClaims>(
            &res.access_token,
            &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .unwrap();
        assert_eq!(decoded.claims.sub, res.user.id.to_string());

        let stored = token_repo.tokens.lock().await;
        assert_eq!(stored.len(), 1);
        assert!(!stored.contains_key(&res.refresh_token));
        assert!(stored.contains_key(&TokenService::hash_refresh_token(&res.refresh_token)));
    }

    #[tokio::test]
    async fn login_succeeds_for_valid_credentials_and_rejects_wrong_password_unknown_email_and_inactive_user(
    ) {
        let user_repo = Arc::new(InMemoryUserRepo::default());
        let token_repo = Arc::new(InMemoryRefreshTokenRepo::default());
        let token_service = Arc::new(TokenService::new(token_repo, test_jwt_config(), 7));
        let sem = test_semaphore();
        let register =
            RegisterUserHandler::new(user_repo.clone(), token_service.clone(), sem.clone());
        let login = LoginUserHandler::new(user_repo.clone(), token_service, sem);

        register
            .handle(RegisterUserCommand {
                email: "siti@example.com".to_string(),
                full_name: "Siti Aminah".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();

        let ok_res = login
            .handle(LoginUserCommand {
                email: "siti@example.com".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(ok_res.user.email, "siti@example.com");

        let wrong_pw = login
            .handle(LoginUserCommand {
                email: "siti@example.com".to_string(),
                password: "wrongpass".to_string(),
            })
            .await;
        assert!(matches!(
            wrong_pw,
            Err(IdentityApplicationError::InvalidCredentials)
        ));

        let unknown = login
            .handle(LoginUserCommand {
                email: "nobody@example.com".to_string(),
                password: "password123".to_string(),
            })
            .await;
        assert!(matches!(
            unknown,
            Err(IdentityApplicationError::InvalidCredentials)
        ));

        let inactive_user = User::reconstitute(
            UserId::new(),
            Email::parse("inactive@example.com").unwrap(),
            FullName::parse("Inactive User").unwrap(),
            ok_res.access_token.clone(),
            false,
            Utc::now(),
            Utc::now(),
        );
        user_repo.save(&inactive_user).await.unwrap();

        let inactive_res = login
            .handle(LoginUserCommand {
                email: "inactive@example.com".to_string(),
                password: "password123".to_string(),
            })
            .await;
        assert!(matches!(
            inactive_res,
            Err(IdentityApplicationError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn refresh_rotates_token_and_rejects_reuse_and_inactive_user() {
        let user_repo = Arc::new(InMemoryUserRepo::default());
        let token_repo = Arc::new(InMemoryRefreshTokenRepo::default());
        let token_service = Arc::new(TokenService::new(token_repo.clone(), test_jwt_config(), 7));
        let register =
            RegisterUserHandler::new(user_repo.clone(), token_service.clone(), test_semaphore());
        let refresh = RefreshTokenHandler::new(user_repo.clone(), token_service.clone());

        let reg = register
            .handle(RegisterUserCommand {
                email: "andi@example.com".to_string(),
                full_name: "Andi Wijaya".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();

        let rotated = refresh
            .handle(RefreshTokenCommand {
                refresh_token: reg.refresh_token.clone(),
            })
            .await
            .unwrap();
        assert_ne!(rotated.refresh_token, reg.refresh_token);

        let reuse = refresh
            .handle(RefreshTokenCommand {
                refresh_token: reg.refresh_token,
            })
            .await;
        assert!(matches!(
            reuse,
            Err(IdentityApplicationError::InvalidRefreshToken)
        ));

        let inactive_id = UserId::new();
        let inactive_user = User::reconstitute(
            inactive_id,
            Email::parse("off@example.com").unwrap(),
            FullName::parse("Off User").unwrap(),
            "hash".to_string(),
            false,
            Utc::now(),
            Utc::now(),
        );
        user_repo.save(&inactive_user).await.unwrap();
        let pair = token_service.issue_token_pair(inactive_id).await.unwrap();

        let inactive_refresh = refresh
            .handle(RefreshTokenCommand {
                refresh_token: pair.refresh_token,
            })
            .await;
        assert!(matches!(
            inactive_refresh,
            Err(IdentityApplicationError::InvalidRefreshToken)
        ));
    }

    #[tokio::test]
    async fn logout_revokes_refresh_token_idempotently() {
        let user_repo = Arc::new(InMemoryUserRepo::default());
        let token_repo = Arc::new(InMemoryRefreshTokenRepo::default());
        let token_service = Arc::new(TokenService::new(token_repo.clone(), test_jwt_config(), 7));
        let register = RegisterUserHandler::new(user_repo, token_service.clone(), test_semaphore());
        let logout = LogoutUserHandler::new(token_service.clone());

        let reg = register
            .handle(RegisterUserCommand {
                email: "rina@example.com".to_string(),
                full_name: "Rina Kusuma".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();

        logout
            .handle(LogoutUserCommand {
                refresh_token: reg.refresh_token.clone(),
            })
            .await
            .unwrap();
        logout
            .handle(LogoutUserCommand {
                refresh_token: reg.refresh_token.clone(),
            })
            .await
            .unwrap();

        let hash = TokenService::hash_refresh_token(&reg.refresh_token);
        let guard = token_repo.tokens.lock().await;
        assert!(guard.get(&hash).unwrap().revoked_at.is_some());
    }

    #[tokio::test]
    async fn get_current_user_returns_active_user_and_rejects_missing_or_inactive() {
        let user_repo = Arc::new(InMemoryUserRepo::default());
        let token_repo = Arc::new(InMemoryRefreshTokenRepo::default());
        let token_service = Arc::new(TokenService::new(token_repo, test_jwt_config(), 7));
        let register = RegisterUserHandler::new(user_repo.clone(), token_service, test_semaphore());
        let me = GetCurrentUserHandler::new(user_repo.clone());

        let reg = register
            .handle(RegisterUserCommand {
                email: "dewi@example.com".to_string(),
                full_name: "Dewi Lestari".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap();

        let found = me
            .handle(GetCurrentUserQuery {
                user_id: UserId::from(reg.user.id),
            })
            .await
            .unwrap();
        assert_eq!(found.email, "dewi@example.com");

        let missing = me
            .handle(GetCurrentUserQuery {
                user_id: UserId::new(),
            })
            .await;
        assert!(matches!(
            missing,
            Err(IdentityApplicationError::InvalidToken)
        ));

        let inactive_id = UserId::new();
        let inactive_user = User::reconstitute(
            inactive_id,
            Email::parse("inactive2@example.com").unwrap(),
            FullName::parse("Inactive Two").unwrap(),
            "hash".to_string(),
            false,
            Utc::now(),
            Utc::now(),
        );
        user_repo.save(&inactive_user).await.unwrap();

        let inactive_res = me
            .handle(GetCurrentUserQuery {
                user_id: inactive_id,
            })
            .await;
        assert!(matches!(
            inactive_res,
            Err(IdentityApplicationError::InvalidToken)
        ));
    }
}
