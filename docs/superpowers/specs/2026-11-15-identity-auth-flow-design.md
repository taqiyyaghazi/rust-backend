# Identity Authentication & Session Lifecycle Design

## 1. Intent and Why
Clients (SPA and mobile apps) need a user to be immediately authenticated right after `POST /api/v1/register` so they can access protected resources without a separate login round-trip. They also need the complete Bearer-token session lifecycle (`login`, `refresh`, `logout`) and a protected resource endpoint (`GET /api/v1/me`) demonstrating stateless cross-context authentication.

## 2. Goals, Non-Goals, and Anti-Goals
- **Goals:**
  - `POST /api/v1/register` (`201 Created`) creates the user and immediately issues an access token + refresh token alongside `UserDto`.
  - `POST /api/v1/login` (`200 OK`) verifies email + password and issues an access token + refresh token alongside `UserDto`.
  - `POST /api/v1/refresh` (`200 OK`) atomically rotates a valid refresh token and issues a new access token + refresh token pair.
  - `POST /api/v1/logout` (`204 No Content`) revokes a refresh token idempotently.
  - `GET /api/v1/me` (`200 OK`) returns `UserDto` for the authenticated caller using a shared `AuthenticatedUser` Axum extractor in `shared_kernel`.
- **Non-Goals:**
  - Cookie-based auth, OAuth/SSO, email verification, password reset, RBAC/roles, or multi-device session listing UI.
- **Anti-Goals:**
  - Bounded contexts outside `identity` querying `identity` database tables to validate access tokens.
  - Storing raw (unhashed) refresh tokens in PostgreSQL.

## 3. Constraints
- Maintain strict DDD 4-layer structure in `crates/identity` (`domain`, `application`, `infrastructure`, `presentation`) and shared HTTP utilities in `crates/shared_kernel`.
- Keep the existing `ProblemDetails` (`{"errorCode": "..."}`) error response format and strict JSON rejection handling (`INVALID_CONTENT_TYPE`, `PAYLOAD_TOO_LARGE`, `INVALID_JSON_PAYLOAD`).
- Argon2 hashing and verification must stay bounded by a shared `tokio::sync::Semaphore` + `tokio::task::spawn_blocking`.
- Migrations live in `migrations/` and run via `cargo run --bin migrate`.

## 4. Approaches Considered
1. **Hybrid Stateless JWT Access Token + Stateful Refresh Token in Postgres (Selected):**
   - Short-lived HS256 JWT `accessToken` verified statelessly in `shared_kernel`; opaque `refreshToken` stored as SHA-256 hash in `refresh_tokens` table.
   - **Trade-offs:** Zero-DB-query resource authentication across bounded contexts while supporting server-side refresh rotation and logout; already-issued access tokens remain valid until their short TTL expires (default 15 minutes).
   - **Why chosen:** Best fit for a DDD modular monolith consumed by SPA/mobile clients.
2. **Fully Stateful Opaque Sessions in Postgres:**
   - **Trade-offs:** Instant access token revocation on logout, but forces every protected call in every bounded context to query `identity`'s session table.
3. **Pure Stateless JWT (Access + Refresh both stateless):**
   - **Trade-offs:** No new database table, but server-side logout and refresh token revocation are impossible.

## 5. Detailed Design

### 5.1 Configuration (`bins/api/src/config.rs` & `.env.example`) *[Builder Call]*
- `JWT_SECRET`: `String` (defaults to `"dev-only-jwt-secret-change-in-production-32b"` when unset so local/test runs work out-of-the-box; documented in `.env.example`).
- `JWT_ACCESS_TTL_SECS`: `u64` (default `900` = 15 minutes).
- `REFRESH_TOKEN_TTL_DAYS`: `i64` (default `7` = 7 days).

### 5.2 Database Migration (`migrations/20260101000002_create_refresh_tokens_table.sql`) *[Builder Call]*
```sql
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
```

### 5.3 Shared Kernel (`crates/shared_kernel`) *[Builder Call]*
- `ApiError::unauthorized(error_code: &'static str)` -> `StatusCode::UNAUTHORIZED` (`401`).
- `JwtAuthConfig` (or `TokenVerifier`) injected into request extensions via Axum `Extension` layer in `bootstrap.rs`.
- `AuthenticatedUser { pub user_id: Uuid }` implementing `FromRequestParts<S>`:
  - Parses `Authorization: Bearer <token>`.
  - Missing or non-Bearer header -> `ApiError::unauthorized("UNAUTHORIZED")`.
  - Invalid signature, malformed token, or expired `exp` -> `ApiError::unauthorized("INVALID_TOKEN")`.
  - Valid token -> extracts `sub` as `Uuid`.

### 5.4 Identity Domain, Application, and Infrastructure (`crates/identity`) *[Builder Call]*
- **Domain:**
  - `User::reconstitute(id, email, full_name, password_hash, is_active, created_at, updated_at)` for DB hydration.
  - `UserRepository` adds `find_by_email(&self, email: &Email)` and `find_by_id(&self, id: UserId)`.
  - `RefreshToken` entity/model and `RefreshTokenRepository` trait with:
    - `save(&self, token: &RefreshToken) -> Result<(), RepositoryError>`
    - `revoke_and_get_user_id(&self, token_hash: &str, now: DateTime<Utc>) -> Result<Option<UserId>, RepositoryError>` (atomic `UPDATE refresh_tokens SET revoked_at = $2 WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > $2 RETURNING user_id` to prevent concurrent refresh race conditions)
    - `revoke(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepositoryError>` (idempotent `UPDATE refresh_tokens SET revoked_at = $2 WHERE token_hash = $1 AND revoked_at IS NULL`).
- **Application:**
  - Token issuer: generates HS256 JWT (`sub`, `iat`, `exp`) and a 32-byte random (`OsRng`) hex-encoded refresh token whose SHA-256 hex digest is persisted via `RefreshTokenRepository`.
  - `RegisterUserHandler`: after saving `User`, issues initial access + refresh token pair and returns `AuthResponseDto { user: UserDto, access_token, refresh_token, expires_in }`.
  - `LoginUserHandler`: validates email format & password length (returns `401 INVALID_CREDENTIALS` on wrong password, unknown email, or inactive user; performs Argon2 verification inside shared `hash_semaphore` + `spawn_blocking`, using a dummy hash verify when email is not found to mitigate timing attacks), then issues access + refresh token pair (`AuthResponseDto`).
  - `RefreshTokenHandler`: hashes input token with SHA-256, calls atomic `revoke_and_get_user_id`, verifies user exists and `is_active`, issues new `TokenPairDto { access_token, refresh_token, expires_in }`. Otherwise returns `401 INVALID_REFRESH_TOKEN`.
  - `LogoutUserHandler`: hashes input token with SHA-256, calls `revoke`. Idempotent (`204 No Content`).
  - `GetCurrentUserHandler`: loads user by `UserId` from `UserRepository`; if missing or `!is_active`, returns `401 INVALID_TOKEN`; otherwise returns `UserDto`.

### 5.5 HTTP Contract (`crates/identity/src/presentation`)
- `POST /api/v1/register` -> `201 Created`, body `AuthResponseDto` (`{ user, accessToken, refreshToken, expiresIn }`).
- `POST /api/v1/login` -> `200 OK`, request `{ email, password }` (`MISSING_EMAIL`, `MISSING_PASSWORD`, `MISSING_REQUIRED_FIELDS` on `None`; `INVALID_EMAIL`, `INVALID_PASSWORD` on format error; `401 INVALID_CREDENTIALS` on auth failure), body `AuthResponseDto`.
- `POST /api/v1/refresh` -> `200 OK`, request `{ refreshToken }` (`400 MISSING_REFRESH_TOKEN` on `None` or empty/whitespace; `401 INVALID_REFRESH_TOKEN` on invalid/expired/revoked), body `TokenPairDto` (`{ accessToken, refreshToken, expiresIn }`).
- `POST /api/v1/logout` -> `204 No Content`, request `{ refreshToken }` (`400 MISSING_REFRESH_TOKEN` on `None` or empty/whitespace).
- `GET /api/v1/me` -> `200 OK`, requires `Authorization: Bearer <accessToken>`, body `UserDto`.
- Update OpenAPI (`bins/api/src/openapi.rs`) with the new paths, schemas, and Bearer security scheme.

## 6. Left to the Builder
- Exact module/file organization for the new command/query handlers (`login_user.rs`, `refresh_token.rs`, `logout_user.rs`, `get_current_user.rs`) and DTOs inside `crates/identity`.
- Exact crate selection for JWT (`jsonwebtoken`) and SHA-256 (`sha2` / `hex` or format macro) in workspace `Cargo.toml`.
