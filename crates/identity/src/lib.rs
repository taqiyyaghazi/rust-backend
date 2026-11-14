//! Identity Bounded Context (`crates/identity`)
//!
//! Structured into four strict DDD layers:
//! - `domain`: Aggregate Root (`User`), Value Objects (`UserId`, `Email`, `FullName`),
//!   Domain Events (`UserRegisteredEvent`), and `UserRepository` port.
//! - `application`: CQRS Command (`RegisterUser`) and `UserDto`.
//! - `infrastructure`: PostgreSQL (`sqlx`) and In-Memory repository implementations.
//! - `presentation`: Axum HTTP handler (`POST /api/v1/register`), request/response schemas, and router.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
