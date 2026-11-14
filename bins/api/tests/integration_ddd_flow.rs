use api::{bootstrap::build_app, config::AppConfig};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use identity::application::dto::UserDto;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_db(database_url: &str) {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to connect to PostgreSQL for integration test");

    // Clean up test data before running test
    sqlx::query("TRUNCATE TABLE users CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to truncate users table in integration test");
}

fn test_config() -> AppConfig {
    let _ = dotenvy::dotenv();
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://ddd_user:ddd_password@localhost:5432/ddd_backend".to_string()
    });

    AppConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        database_url,
        cors_allowed_origins: vec!["http://localhost:3000".to_string()],
    }
}

#[tokio::test]
async fn register_endpoint_and_removed_routes_flow() {
    let config = test_config();
    setup_test_db(&config.database_url).await;

    let app = build_app(&config)
        .await
        .expect("Failed to build Axum app with Postgres");

    let unique_email = format!("test.{}@example.com", Uuid::new_v4().simple());

    // 1. Register a new User via POST /api/v1/register -> 201 Created
    let register_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": unique_email,
                "full_name": "Andi Wijaya"
            })
            .to_string(),
        ))
        .unwrap();

    let register_res = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_res.status(), StatusCode::CREATED);

    let body_bytes = register_res.into_body().collect().await.unwrap().to_bytes();
    let created_user: UserDto = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(created_user.email, unique_email);
    assert!(created_user.is_active);

    // 2. Duplicate email -> 409 Conflict
    let duplicate_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": unique_email,
                "full_name": "Andi Wijaya"
            })
            .to_string(),
        ))
        .unwrap();

    let duplicate_res = app.clone().oneshot(duplicate_req).await.unwrap();
    assert_eq!(duplicate_res.status(), StatusCode::CONFLICT);

    // 3. Invalid email -> 400 Bad Request
    let invalid_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "not-an-email",
                "full_name": "Andi"
            })
            .to_string(),
        ))
        .unwrap();

    let invalid_res = app.clone().oneshot(invalid_req).await.unwrap();
    assert_eq!(invalid_res.status(), StatusCode::BAD_REQUEST);

    // 4. Health check remains available -> 200 OK
    let health_req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let health_res = app.clone().oneshot(health_req).await.unwrap();
    assert_eq!(health_res.status(), StatusCode::OK);

    // 5. Removed endpoints (/api/v1/users, /api/v1/orders) -> 404 Not Found
    let old_users_req = Request::builder()
        .method("GET")
        .uri("/api/v1/users")
        .body(Body::empty())
        .unwrap();
    let old_users_res = app.clone().oneshot(old_users_req).await.unwrap();
    assert_eq!(old_users_res.status(), StatusCode::NOT_FOUND);

    let old_orders_req = Request::builder()
        .method("POST")
        .uri("/api/v1/orders")
        .body(Body::empty())
        .unwrap();
    let old_orders_res = app.clone().oneshot(old_orders_req).await.unwrap();
    assert_eq!(old_orders_res.status(), StatusCode::NOT_FOUND);

    // 6. Concurrent duplicate registration -> one 201 Created, one 409 Conflict (never 500)
    let race_email = format!("race.{}@example.com", Uuid::new_v4().simple());
    let make_race_req = || {
        Request::builder()
            .method("POST")
            .uri("/api/v1/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": race_email,
                    "full_name": "Budi Santoso"
                })
                .to_string(),
            ))
            .unwrap()
    };

    let (res_a, res_b) = tokio::join!(
        app.clone().oneshot(make_race_req()),
        app.clone().oneshot(make_race_req())
    );
    let mut statuses = [res_a.unwrap().status(), res_b.unwrap().status()];
    statuses.sort_by_key(|s| s.as_u16());
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CONFLICT]);

    // 7. CORS allows configured origin and omits disallowed origin
    let allowed_cors_req = Request::builder()
        .method("GET")
        .uri("/health")
        .header("origin", "http://localhost:3000")
        .body(Body::empty())
        .unwrap();
    let allowed_cors_res = app.clone().oneshot(allowed_cors_req).await.unwrap();
    assert_eq!(
        allowed_cors_res
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost:3000")
    );

    let blocked_cors_req = Request::builder()
        .method("GET")
        .uri("/health")
        .header("origin", "https://evil.example.com")
        .body(Body::empty())
        .unwrap();
    let blocked_cors_res = app.oneshot(blocked_cors_req).await.unwrap();
    assert!(blocked_cors_res
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}
