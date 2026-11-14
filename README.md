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
│   └── 20260101000001_create_users_table.sql
├── crates/
│   ├── shared_kernel/          # Building blocks DDD lintas Bounded Context
│   │   └── src/
│   │       ├── domain/         # DomainEvent trait & DomainEventEnvelope
│   │       ├── application/    # EventBus & EventSubscriber trait
│   │       ├── infrastructure/ # InMemoryEventBus & AuditLogEventSubscriber
│   │       └── presentation/   # RFC 7807 ProblemDetails & ApiError
│   └── identity/               # Bounded Context: Identity & User Registration
│       └── src/
│           ├── domain/         # User (Aggregate Root), Email/FullName/UserId (VO), UserRepository trait
│           ├── application/    # RegisterUserCommand, RegisterUserHandler, UserDto
│           ├── infrastructure/ # PostgresUserRepository (sqlx murni)
│           └── presentation/   # Axum handler (POST /api/v1/register) & identity_router
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
- **Swagger UI**: `http://localhost:8080/swagger-ui`
- **OpenAPI JSON**: `http://localhost:8080/api-docs/openapi.json`
- **Health Check**: `http://localhost:8080/health`
