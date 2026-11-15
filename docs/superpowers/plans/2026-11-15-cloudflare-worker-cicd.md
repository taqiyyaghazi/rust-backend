# Cloudflare Workers + D1 Dual-Target & CI/CD Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable the Rust DDD Modular Monolith backend to compile and run on Cloudflare Workers Free Tier + Cloudflare D1 (`wasm32-unknown-unknown`) alongside the existing PostgreSQL/Docker binary, with automated GitHub Actions CI/CD for Staging (on PR) and Production (on `main`).

**Architecture:** We keep a Dual-Target Cargo Workspace where `crates/identity` gates PostgreSQL (`sqlx`) behind feature `postgres` (default) and Cloudflare D1 (`worker`) behind feature `d1`. Cross-target password hashing uses `spawn_blocking` + standard Argon2id on native targets and synchronous lightweight Argon2id (`m=1024, t=1, p=1`) on `wasm32` to stay under the 10ms CPU Free Tier limit. A new `bins/worker` (`cdylib`) crate mounts the shared `identity_router` Axum router over `worker`'s HTTP fetch event and connects to Cloudflare D1.

**Tech Stack:** Rust 1.89, Axum 0.8, `worker` (`workers-rs` 0.7 with `http` + `d1` features), Cloudflare D1 (SQLite), Wrangler, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-15-cloudflare-worker-cicd-design.md`

## Global Constraints

- Rust version floor: `1.89`, edition `2021`.
- Cloudflare Workers Free Tier CPU limit: maximum 10ms CPU time per request (`wasm32` Argon2id parameters: `m_cost = 1024`, `t_cost = 1`, `p_cost = 1`).
- Existing `bins/api`, `Dockerfile`, `docker-compose.yml`, and `migrations/` (PostgreSQL) must remain intact and pass all existing tests.
- All HTTP error responses must continue returning JSON `{"errorCode": "..."}` via `shared_kernel::presentation::ApiError`.
- `JWT_SECRET` must be validated to be at least 32 characters on startup/request handling (`config` / worker bootstrap).

## Review Focus

1. **D1 SQLite row deserialization & boolean/timestamp parsing**: SQLite stores booleans as `INTEGER` (`0`/`1`) and timestamps as RFC3339 `TEXT`; `D1UserRow` must accept both integer (`0`/`1`) and boolean JSON representations from D1 and parse RFC3339 `created_at`/`updated_at` strings into `DateTime<Utc>`.
2. **D1 unique constraint detection**: When inserting a duplicate email or token hash into D1, Cloudflare D1 returns an error string containing `"UNIQUE constraint failed"` — `is_d1_unique_violation` must map this to `RepositoryError::Conflict` so `POST /api/v1/register` returns `409 Conflict` (`EMAIL_ALREADY_EXISTS`) instead of `500`.
3. **Cross-target Argon2id verification compatibility**: `password_hasher::hash_password` and `password_hasher::verify_password` must verify valid passwords, reject wrong passwords, and run dummy verification when `user_opt` is `None` without panicking on either native or `wasm32` params.
4. **Worker environment configuration validation**: Missing or short (<32 chars) `JWT_SECRET` in Cloudflare Worker `Env` must fail cleanly with a descriptive error rather than silently issuing tokens with an empty or weak key.
5. **GitHub Actions PR security guard**: The `deploy-staging` job must only run on internal repository PRs (`github.event.pull_request.head.repo.full_name == github.repository`) so fork PRs without secrets do not fail CI.

---

### Task 1: Wasm-Compatible Dependencies & Cross-Target Password Hasher in `shared_kernel` and `identity`

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/shared_kernel/Cargo.toml`
- Modify: `crates/identity/Cargo.toml`
- Create: `crates/identity/src/application/services/password_hasher.rs`
- Modify: `crates/identity/src/application/services/mod.rs`
- Modify: `crates/identity/src/application/commands/register_user.rs`
- Modify: `crates/identity/src/application/commands/login_user.rs`
- Modify: `crates/identity/src/presentation/state.rs`

**Interfaces:**
- Consumes: `IdentityApplicationError::PasswordHashing` from `crates/identity/src/application/errors.rs`
- Produces:
  - `pub async fn hash_password(raw_password: String, semaphore: &Arc<tokio::sync::Semaphore>) -> Result<String, IdentityApplicationError>` in `crates/identity/src/application/services/password_hasher.rs`
  - `pub async fn verify_password(raw_password: String, target_hash: Option<String>, semaphore: &Arc<tokio::sync::Semaphore>) -> Result<bool, IdentityApplicationError>` in `crates/identity/src/application/services/password_hasher.rs`
  - Cargo feature flags on `identity`: `default = ["postgres"]`, `postgres = ["dep:sqlx"]`, `d1 = ["dep:worker", "dep:serde_json"]`

- [ ] **Step 1: Write the failing unit tests for `password_hasher` in `crates/identity/src/application/services/password_hasher.rs`**

```rust
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

        let valid = verify_password("correct-horse-battery".to_string(), Some(hash.clone()), &sem)
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p identity password_hasher`
Expected: FAIL (module `password_hasher` does not exist yet)

- [ ] **Step 3: Update `Cargo.toml`, `crates/shared_kernel/Cargo.toml`, and `crates/identity/Cargo.toml` for Wasm compatibility and feature flags, then implement `password_hasher.rs` and wire it into `register_user.rs`, `login_user.rs`, and `state.rs`**

In root `Cargo.toml`:
- Add `worker = { version = "0.7", features = ["http", "d1"] }` and `getrandom = { version = "0.2", features = ["js"] }` to `[workspace.dependencies]`.

In `crates/shared_kernel/Cargo.toml`:
- Add target-specific dependencies for `wasm32`:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { workspace = true }
uuid = { version = "1.10", features = ["v4", "serde", "js"] }
chrono = { version = "0.4", features = ["serde", "wasmbind"] }
```

In `crates/identity/Cargo.toml`:
- Make `sqlx` optional (`sqlx = { workspace = true, optional = true }`), add optional `worker = { workspace = true, optional = true }` and `serde_json = { workspace = true, optional = true }`.
- Change `tokio` dependency to `tokio = { version = "1.40", default-features = false, features = ["sync"] }` and under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` add `tokio = { workspace = true }`.
- Under `[target.'cfg(target_arch = "wasm32")'.dependencies]` add `getrandom = { workspace = true }`, `uuid = { version = "1.10", features = ["v4", "serde", "js"] }`, and `chrono = { version = "0.4", features = ["serde", "wasmbind"] }`.
- Define `[features]`:
```toml
[features]
default = ["postgres"]
postgres = ["dep:sqlx"]
d1 = ["dep:worker", "dep:serde_json"]
```

In `crates/identity/src/application/services/password_hasher.rs`:
- Implement `pub(crate) fn hash_password_with_params(raw_password: &[u8], m_cost: u32, t_cost: u32, p_cost: u32) -> Result<String, IdentityApplicationError>` using `argon2::{Algorithm, Argon2, Params, Version, password_hash::{PasswordHasher, SaltString, rand_core::OsRng}}`.
- Implement `pub async fn hash_password(raw_password: String, semaphore: &Arc<tokio::sync::Semaphore>) -> Result<String, IdentityApplicationError>`:
  - On `#[cfg(not(target_arch = "wasm32"))]`: acquire `semaphore`, run `tokio::task::spawn_blocking` with `Argon2::default()`.
  - On `#[cfg(target_arch = "wasm32")]`: acquire `semaphore`, call `hash_password_with_params(raw_password.as_bytes(), 1024, 1, 1)` directly.
- Implement `pub async fn verify_password(raw_password: String, target_hash: Option<String>, semaphore: &Arc<tokio::sync::Semaphore>) -> Result<bool, IdentityApplicationError>`:
  - Use `DUMMY_PASSWORD_HASH_NATIVE` (`$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA`) on `not(target_arch = "wasm32")` and `DUMMY_PASSWORD_HASH_WASM` (`$argon2id$v=19$m=1024,t=1,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA`) on `wasm32` when `target_hash` is `None`.
  - Update `register_user.rs` and `login_user.rs` to call `password_hasher::hash_password` and `password_hasher::verify_password`.
  - In `crates/identity/src/presentation/state.rs`, gate `std::thread::available_parallelism()` with `#[cfg(not(target_arch = "wasm32"))]` and use `1` on `#[cfg(target_arch = "wasm32")]`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p identity && cargo test -p shared_kernel`
Expected: PASS (all unit tests in `identity` and `shared_kernel` pass)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/shared_kernel/Cargo.toml crates/identity/
git commit -m "feat(identity): add cross-target password hasher and wasm32 feature flags"
```

---

### Task 2: Cloudflare D1 Migrations & D1 Repositories (`D1UserRepository` & `D1RefreshTokenRepository`)

**Files:**
- Create: `migrations_d1/0001_create_users_table.sql`
- Create: `migrations_d1/0002_create_refresh_tokens_table.sql`
- Create: `crates/identity/src/infrastructure/persistence/d1_user_repository.rs`
- Create: `crates/identity/src/infrastructure/persistence/d1_refresh_token_repository.rs`
- Modify: `crates/identity/src/infrastructure/persistence/mod.rs`
- Modify: `crates/identity/src/infrastructure/mod.rs`

**Interfaces:**
- Consumes: `UserRepository`, `RefreshTokenRepository`, `RepositoryError`, `User`, `RefreshToken`, `Email`, `FullName`, `UserId` from `crates/identity/src/domain/`
- Produces:
  - `pub struct D1UserRepository` with `pub fn new(db: Arc<worker::D1Database>) -> Self` implementing `UserRepository` (gated by `#[cfg(feature = "d1")]`)
  - `pub struct D1RefreshTokenRepository` with `pub fn new(db: Arc<worker::D1Database>) -> Self` implementing `RefreshTokenRepository` (gated by `#[cfg(feature = "d1")]`)
  - `pub(crate) fn is_d1_unique_violation(msg: &str) -> bool` and `D1UserRow::into_domain(self) -> Result<User, RepositoryError>`

- [ ] **Step 1: Write the failing unit tests for D1 row conversion and unique constraint mapping in `crates/identity/src/infrastructure/persistence/d1_user_repository.rs`**

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p identity --features d1 d1_user_repository`
Expected: FAIL (`d1_user_repository` module does not exist yet)

- [ ] **Step 3: Create `migrations_d1/` SQL files and implement `D1UserRepository` and `D1RefreshTokenRepository`**

1. Create `migrations_d1/0001_create_users_table.sql` and `migrations_d1/0002_create_refresh_tokens_table.sql` with the exact SQLite schemas from Spec Section 5.3.
2. In `crates/identity/src/infrastructure/persistence/d1_user_repository.rs`:
   - Define `pub(crate) struct SendD1Database(pub Arc<worker::D1Database>);` with `unsafe impl Send for SendD1Database {}` and `unsafe impl Sync for SendD1Database {}` (safe because Cloudflare Workers `wasm32` runtime is single-threaded).
   - Define `D1UserRow` with custom deserializer for `is_active` accepting either `i64`/`u64` (`!= 0`) or `bool`, and `into_domain(self) -> Result<User, RepositoryError>` parsing `Uuid`, `Email`, `FullName`, and `DateTime::parse_from_rfc3339`.
   - Implement `UserRepository for D1UserRepository` wrapping async D1 calls in `worker::send::SendFuture::new(async move { ... })`.
3. In `crates/identity/src/infrastructure/persistence/d1_refresh_token_repository.rs`:
   - Implement `RefreshTokenRepository for D1RefreshTokenRepository` using `SendD1Database` and `worker::send::SendFuture::new(...)` for `save`, `revoke_and_get_user_id` (`UPDATE refresh_tokens SET revoked_at = ?2 WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2 RETURNING user_id`), and `revoke`.
4. Gate `postgres_*` modules in `crates/identity/src/infrastructure/persistence/mod.rs` and `crates/identity/src/infrastructure/mod.rs` with `#[cfg(feature = "postgres")]` and gate `d1_*` modules with `#[cfg(feature = "d1")]`.

- [ ] **Step 4: Run tests and check `wasm32-unknown-unknown` compilation for `identity`**

Run: `cargo test -p identity --features d1 && cargo check -p identity --no-default-features --features d1 --target wasm32-unknown-unknown`
Expected: PASS (unit tests pass and `identity` compiles cleanly for `wasm32-unknown-unknown`)

- [ ] **Step 5: Commit**

```bash
git add migrations_d1/ crates/identity/
git commit -m "feat(identity): implement Cloudflare D1 repositories and SQLite migrations"
```

---

### Task 3: Create `bins/worker` Crate & `wrangler.toml` for Cloudflare Workers

**Files:**
- Create: `bins/worker/Cargo.toml`
- Create: `bins/worker/src/lib.rs`
- Create: `bins/worker/src/openapi.rs`
- Create: `wrangler.toml`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `D1UserRepository`, `D1RefreshTokenRepository`, `identity_router`, `IdentityHttpState` from `identity`; `ApiError`, `JwtAuthConfig`, `ProblemDetails` from `shared_kernel`
- Produces:
  - `pub fn parse_worker_settings(jwt_secret: &str, access_ttl_raw: Option<&str>, refresh_ttl_raw: Option<&str>, cors_origins_raw: Option<&str>) -> Result<WorkerSettings, String>` in `bins/worker/src/lib.rs`
  - Cloudflare Worker `#[event(fetch)]` entrypoint in `bins/worker/src/lib.rs`

- [ ] **Step 1: Write the failing unit tests for `parse_worker_settings` in `bins/worker/src/lib.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_jwt_secret_shorter_than_32_chars() {
        let err = parse_worker_settings("short-secret", None, None, None).unwrap_err();
        assert!(err.contains("at least 32 characters"));
    }

    #[test]
    fn parses_defaults_and_comma_separated_cors_origins() {
        let settings = parse_worker_settings(
            "0123456789abcdef0123456789abcdef",
            None,
            None,
            Some("https://staging.example.com, https://app.example.com "),
        )
        .expect("valid settings");

        assert_eq!(settings.jwt_access_ttl_secs, 900);
        assert_eq!(settings.refresh_token_ttl_days, 30);
        assert_eq!(
            settings.cors_allowed_origins,
            vec![
                "https://staging.example.com".to_string(),
                "https://app.example.com".to_string()
            ]
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker`
Expected: FAIL (package `worker` in `bins/worker` does not exist yet)

- [ ] **Step 3: Create `bins/worker/Cargo.toml`, `bins/worker/src/openapi.rs`, `bins/worker/src/lib.rs`, `wrangler.toml`, and register `bins/worker` in root `Cargo.toml`**

1. In root `Cargo.toml`:
   - Add `"bins/worker"` to `[workspace] members` and add `default-members = ["crates/shared_kernel", "crates/identity", "bins/api"]` so standard native `cargo build`/`cargo test` runs native members by default while `-p worker` targets the worker crate.
2. In `bins/worker/Cargo.toml`:
   - Package name `rust_backend_worker` (or `worker_api` to avoid colliding with the `worker` dependency crate name), `[lib] crate-type = ["cdylib", "rlib"]`.
   - Dependencies: `identity = { path = "../../crates/identity", default-features = false, features = ["d1"] }`, `shared_kernel = { workspace = true }`, `axum = { workspace = true, default-features = false, features = ["json", "macros"] }`, `tower-http = { workspace = true }`, `tower-service = "0.3"`, `worker = { workspace = true }`, `serde_json = { workspace = true }`, `utoipa = { workspace = true }`, `http = "1"`.
3. In `bins/worker/src/openapi.rs`:
   - Define `pub struct ApiDoc;` with `#[derive(OpenApi)]` mirroring `bins/api/src/openapi.rs` (without `utoipa-swagger-ui`).
4. In `bins/worker/src/lib.rs`:
   - Implement `WorkerSettings` and `pub fn parse_worker_settings(...) -> Result<WorkerSettings, String>` validating `jwt_secret.len() >= 32` and `refresh_token_ttl_days > 0`.
   - Implement `GET /health` querying `SELECT 1` on D1 and returning `{"status": "ok", "architecture": "ddd-modular-monolith", "bounded_contexts": ["identity"], "storage": "cloudflare-d1"}`.
   - Implement `GET /api-docs/openapi.json` returning `Json(ApiDoc::openapi())`.
   - Merge `identity_router(identity_state)` with `DefaultBodyLimit::max(16 * 1024)`, `CorsLayer`, `Extension(jwt_config)`, `fallback(|| async { ApiError::not_found() })`, and `method_not_allowed_fallback(|| async { ApiError::method_not_allowed() })`.
   - Implement `#[event(fetch)] pub async fn fetch(req: worker::HttpRequest, env: Env, _ctx: Context) -> worker::Result<http::Response<axum::body::Body>>` calling `tower_service::Service::call(&mut router, req).await`.
5. Create `wrangler.toml` at repository root with `[build] command = "cargo install -q worker-build && worker-build --release"` (with `cwd = "bins/worker"`), `main = "bins/worker/build/worker/shim.mjs"`, `[env.staging]`, and `[env.production]` with D1 database bindings (`binding = "DB"`, `migrations_dir = "migrations_d1"`).

- [ ] **Step 4: Run unit tests and verify `wasm32-unknown-unknown` compilation for `bins/worker`**

Run: `cargo test -p rust_backend_worker && cargo check -p rust_backend_worker --target wasm32-unknown-unknown && cargo clippy -p rust_backend_worker --target wasm32-unknown-unknown -- -D warnings`
Expected: PASS (unit tests pass and `rust_backend_worker` compiles cleanly to `wasm32-unknown-unknown` with zero clippy warnings)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock bins/worker/ wrangler.toml
git commit -m "feat(worker): add Cloudflare Worker entrypoint crate and wrangler config"
```

---

### Task 4: GitHub Actions CI/CD Workflow (`.github/workflows/ci-cd.yml`) & Setup Script/Docs

**Files:**
- Create: `.github/workflows/ci-cd.yml`
- Create: `scripts/setup_cloudflare_d1.sh`
- Modify: `README.md`

**Interfaces:**
- Consumes: `bins/api`, `bins/worker` (`rust_backend_worker`), `wrangler.toml`, `migrations/`, `migrations_d1/`
- Produces:
  - `.github/workflows/ci-cd.yml` with jobs `ci`, `deploy-staging`, and `deploy-production`
  - `scripts/setup_cloudflare_d1.sh` helper script for provisioning Staging & Production D1 databases and secrets

- [ ] **Step 1: Create `.github/workflows/ci-cd.yml` and `scripts/setup_cloudflare_d1.sh`**

1. Create `.github/workflows/ci-cd.yml` implementing Pola A from Spec Section 5.6:
   - Trigger on `push` to `main` and `pull_request` targeting `main`.
   - Job `ci`:
     - `services.postgres` (`postgres:16-alpine`, user `postgres`, password `postgres`, db `app_db`, port `5432:5432`, healthcheck `pg_isready`).
     - Install Rust stable (`1.89`) with `target: wasm32-unknown-unknown` and components `rustfmt, clippy`.
     - Run `cargo fmt --all -- --check`.
     - Run `cargo clippy --workspace --exclude rust_backend_worker --all-targets -- -D warnings`.
     - Run `cargo run --bin migrate` and `cargo test --workspace --exclude rust_backend_worker`.
     - Run `cargo test -p identity --features d1` and `cargo test -p rust_backend_worker`.
     - Run `cargo clippy -p rust_backend_worker --target wasm32-unknown-unknown -- -D warnings`.
   - Job `deploy-staging`:
     - `needs: ci`
     - Condition: `if: github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository`
     - `environment: staging` and `concurrency: { group: staging-deploy, cancel-in-progress: false }`
     - Install Rust + `wasm32-unknown-unknown` + `worker-build`
     - Run D1 migrations on staging: `npx wrangler d1 migrations apply rust-backend-db-staging --env staging --remote`
     - Deploy Worker to staging: `npx wrangler deploy --env staging`
   - Job `deploy-production`:
     - `needs: ci`
     - Condition: `if: github.event_name == 'push' && github.ref == 'refs/heads/main'`
     - `environment: production` and `concurrency: { group: production-deploy, cancel-in-progress: false }`
     - Install Rust + `wasm32-unknown-unknown` + `worker-build`
     - Run D1 migrations on production: `npx wrangler d1 migrations apply rust-backend-db-prod --env production --remote`
     - Deploy Worker to production: `npx wrangler deploy --env production`
2. Create executable `scripts/setup_cloudflare_d1.sh` to help initialize `rust-backend-db-staging` and `rust-backend-db-prod` via `wrangler d1 create` and set `JWT_SECRET` via `wrangler secret put`.
3. Update `README.md` with documentation on the Dual-Target architecture, how to run locally with Wrangler (`npx wrangler dev --env staging`), required GitHub Secrets (`CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`), and how the CI/CD pipeline works.

- [ ] **Step 2: Verify formatting, clippy, native tests, and wasm32 build across the entire repository**

Run:
```bash
cargo fmt --all -- --check && \
cargo clippy --workspace --exclude rust_backend_worker --all-targets -- -D warnings && \
cargo clippy -p rust_backend_worker --target wasm32-unknown-unknown -- -D warnings && \
cargo test --lib --bins && \
cargo test -p identity --features d1 && \
cargo test -p rust_backend_worker
```
Expected: PASS with 0 warnings and all unit tests green.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci-cd.yml scripts/setup_cloudflare_d1.sh README.md
git commit -m "ci(cloudflare): add GitHub Actions CI/CD workflow for D1 migrations and Worker deploy"
```
