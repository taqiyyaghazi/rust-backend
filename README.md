# Rust Domain-Driven Design (DDD) Modular Monolith Backend

Proyek backend **Rust** menggunakan arsitektur **Domain-Driven Design (DDD)** dengan pola **Cargo Workspace Modular Monolith**, **CQRS-lite Register User Use Case**, **PostgreSQL murni (SQLx)**, **Axum**, dan **OpenAPI / Swagger UI**.

---

## Struktur Workspace & Layer DDD

```text
.
├── Cargo.toml                  # Root Cargo Workspace manifest
├── docker-compose.yml          # PostgreSQL 16 container orchestration
├── Dockerfile                  # Multi-stage production container build
├── .env.example                # Contoh konfigurasi environment (Postgres & server)
├── scripts/
│   ├── migrate.sh              # Script mandiri untuk mengeksekusi migrasi database
│   └── test_with_docker.sh     # Menjalankan Docker Compose -> Migrasi -> Integration Test
├── migrations/                 # SQLx PostgreSQL migrations
│   ├── 20260101000001_create_users_table.sql
│   └── 20260101000002_create_refresh_tokens_table.sql
├── crates/
│   ├── shared_kernel/          # Building blocks lintas Bounded Context
│   │   └── src/
│   │       └── presentation/   # ApiError, ProblemDetails & AuthenticatedUser JWT extractor
│   └── identity/               # Bounded Context: Identity & Session Lifecycle
│       └── src/
│           ├── domain/         # User, RefreshToken, Value Objects, Repository traits
│           ├── application/    # Register/Login/Refresh/Logout commands, GetCurrentUser query, TokenService
│           ├── infrastructure/ # PostgresUserRepository & PostgresRefreshTokenRepository (sqlx)
│           └── presentation/   # Axum handlers (/register, /login, /refresh, /logout, /me) & identity_router
└── bins/
    └── api/                    # Composition Root (Binary utama & Runner Migrasi)
        ├── src/
        │   ├── main.rs         # Entrypoint server API Tokio + Tracing
        │   ├── bin/
        │   │   └── migrate.rs  # Binary mandiri untuk menjalankan migrasi SQLx
        │   ├── bootstrap.rs    # Injeksi dependensi PostgresUserRepository & Axum Router
        │   ├── config.rs       # Konfigurasi database & port (.env)
        │   └── openapi.rs      # Dokumentasi OpenAPI / Swagger UI
        └── tests/
            └── integration_ddd_flow.rs # Integration test langsung ke PostgreSQL
```

---

## Alur Menjalankan Database & Aplikasi

### 1. Menjalankan Database PostgreSQL
```bash
cp .env.example .env
docker compose up -d postgres
```

### 2. Menjalankan Migrasi Database Secara Mandiri
Migrasi **TIDAK** dijalankan otomatis oleh API server. Jalankan migrasi secara terpisah sebelum menyalakan server:
```bash
./scripts/migrate.sh
# atau langsung via Cargo:
cargo run --bin migrate
```

### 3. Menjalankan API Server
```bash
cargo run --bin api
```

### 4. Menjalankan Integration Test
Untuk menjalankan alur lengkap otomatis (**Docker Compose -> Migrasi -> Integration Test**):
```bash
./scripts/test_with_docker.sh
```
Atau manual:
```bash
cargo test --workspace
```

---

## Endpoint API Tersedia
- **Register User**: `POST http://localhost:8080/api/v1/register`
  - Request body (`camelCase`): `{"email": "...", "fullName": "...", "password": "..."}` (password 8–128 karakter, disimpan sebagai hash Argon2)
  - Response `201 Created`: `{"user": {...}, "accessToken": "...", "refreshToken": "...", "expiresIn": 900}`
- **Login**: `POST http://localhost:8080/api/v1/login`
  - Request body: `{"email": "...", "password": "..."}` → Response `200 OK` dengan pasangan `accessToken` & `refreshToken`
- **Refresh Token**: `POST http://localhost:8080/api/v1/refresh`
  - Request body: `{"refreshToken": "..."}` → Rotasi atomik token lama, Response `200 OK` dengan pasangan token baru
- **Logout**: `POST http://localhost:8080/api/v1/logout`
  - Request body: `{"refreshToken": "..."}` → Mencabut refresh token secara idempoten (`204 No Content`)
- **Current User**: `GET http://localhost:8080/api/v1/me`
  - Membutuhkan header `Authorization: Bearer <accessToken>` → Response `200 OK` (`UserDto`)
- **Catatan**:
  - Response error (semua status error mengembalikan JSON): `{"errorCode": "..."}`
  - CORS dikontrol melalui environment variable `CORS_ALLOWED_ORIGINS` (daftar origin dipisahkan koma di `.env`)
  - `JWT_SECRET` wajib diisi minimal 32 karakter (lihat `.env.example`)
- **Swagger UI**: `http://localhost:8080/swagger-ui`
- **OpenAPI JSON**: `http://localhost:8080/api-docs/openapi.json`
- **Health Check**: `http://localhost:8080/health`

---

## Deployment ke Cloudflare Workers + D1 (Free Tier)

Proyek ini mendukung **Dual-Target** dalam satu Cargo Workspace:
- `bins/api`: Server native Axum + PostgreSQL (`sqlx`) + Docker.
- `bins/worker`: Cloudflare Worker (`wasm32-unknown-unknown`) + **Cloudflare D1 (SQLite)** dengan rute API yang sama (`identity_router`).

### 1. Inisialisasi Database D1 & Secrets
Jalankan helper script atau perintah `wrangler` berikut satu kali:
```bash
./scripts/setup_cloudflare_d1.sh
```
Lalu salin masing-masing `database_id` ke `wrangler.toml` (`[env.staging]` dan `[env.production]`), dan set `JWT_SECRET`:
```bash
npx wrangler@3 secret put JWT_SECRET --env staging
npx wrangler@3 secret put JWT_SECRET --env production
```

### 2. Menjalankan Worker Secara Lokal
```bash
npx wrangler@3 d1 migrations apply rust-backend-db-staging --local --env staging
npx wrangler@3 dev --env staging
```

### 3. CI/CD Pipeline (GitHub Actions)
Workflow berada di `.github/workflows/ci-cd.yml`:
- **Pull Request ke `main`**: Menjalankan `cargo fmt`, `cargo clippy`, unit + integration test PostgreSQL, cek kompilasi `wasm32-unknown-unknown`, lalu otomatis menjalankan migrasi D1 Staging & deploy ke Worker **Staging** (khusus PR internal).
- **Push / Merge ke `main`**: Otomatis menjalankan migrasi D1 Production & deploy ke Worker **Production**.

Pastikan GitHub Repository Secrets berikut sudah diisi:
- `CLOUDFLARE_API_TOKEN` (izin *Workers Scripts: Edit* dan *D1: Edit*)
- `CLOUDFLARE_ACCOUNT_ID`

