use api::{bootstrap::build_app, config::AppConfig};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use identity::application::dto::{AuthResponseDto, TokenPairDto, UserDto};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_db(database_url: &str) {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to connect to PostgreSQL for integration test");

    // Clean up test data before running test
    sqlx::query("TRUNCATE TABLE refresh_tokens, users CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to truncate tables in integration test");
}

fn test_config() -> AppConfig {
    let mut config = AppConfig::from_env();
    config.host = "127.0.0.1".to_string();
    config.port = 0;
    config.cors_allowed_origins = vec!["http://localhost:3000".to_string()];
    config.jwt_secret = "test-only-jwt-secret-at-least-32-bytes".to_string();
    config
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
                "fullName": "Andi Wijaya",
                "password": "secret123"
            })
            .to_string(),
        ))
        .unwrap();

    let register_res = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_res.status(), StatusCode::CREATED);

    let body_bytes = register_res.into_body().collect().await.unwrap().to_bytes();
    let reg_auth: AuthResponseDto = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(reg_auth.user.email, unique_email);
    assert!(reg_auth.user.is_active);
    assert!(!reg_auth.access_token.is_empty());
    assert!(!reg_auth.refresh_token.is_empty());
    assert_eq!(reg_auth.expires_in, 900);

    // 1b. Immediately access protected GET /api/v1/me using access_token from register
    let me_req = Request::builder()
        .method("GET")
        .uri("/api/v1/me")
        .header("authorization", format!("Bearer {}", reg_auth.access_token))
        .body(Body::empty())
        .unwrap();
    let me_res = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(me_res.status(), StatusCode::OK);
    let me_body: UserDto =
        serde_json::from_slice(&me_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(me_body.id, reg_auth.user.id);
    assert_eq!(me_body.email, unique_email);

    // 1c. GET /api/v1/me without token -> 401 UNAUTHORIZED; invalid token -> 401 INVALID_TOKEN
    let me_unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_unauth.status(), StatusCode::UNAUTHORIZED);
    let me_unauth_body: serde_json::Value = serde_json::from_slice(
        &me_unauth.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(me_unauth_body["errorCode"], "UNAUTHORIZED");

    let me_bad_token = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me")
                .header("authorization", "Bearer bad.jwt.token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_bad_token.status(), StatusCode::UNAUTHORIZED);
    let me_bad_body: serde_json::Value = serde_json::from_slice(
        &me_bad_token.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(me_bad_body["errorCode"], "INVALID_TOKEN");

    // 2. Duplicate email -> 409 Conflict
    let duplicate_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": unique_email,
                "fullName": "Andi Wijaya",
                "password": "secret123"
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
                "fullName": "Andi",
                "password": "secret123"
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
    let old_users_body: serde_json::Value = serde_json::from_slice(
        &old_users_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(old_users_body["errorCode"], "NOT_FOUND");

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
                    "fullName": "Budi Santoso",
                    "password": "secret123"
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
    let blocked_cors_res = app.clone().oneshot(blocked_cors_req).await.unwrap();
    assert!(blocked_cors_res
        .headers()
        .get("access-control-allow-origin")
        .is_none());

    // 8. Missing Content-Type / empty body and null JSON return JSON ProblemDetails (400)
    let no_ct_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .body(Body::empty())
        .unwrap();
    let no_ct_res = app.clone().oneshot(no_ct_req).await.unwrap();
    assert_eq!(no_ct_res.status(), StatusCode::BAD_REQUEST);
    let no_ct_body: serde_json::Value = serde_json::from_slice(
        &no_ct_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(no_ct_body["errorCode"], "INVALID_CONTENT_TYPE");

    let null_json_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from("null"))
        .unwrap();
    let null_json_res = app.clone().oneshot(null_json_req).await.unwrap();
    assert_eq!(null_json_res.status(), StatusCode::BAD_REQUEST);
    let null_json_body: serde_json::Value = serde_json::from_slice(
        &null_json_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(null_json_body["errorCode"], "INVALID_JSON_PAYLOAD");

    // 9. Missing fields return specific error codes
    let empty_obj_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let empty_obj_res = app.clone().oneshot(empty_obj_req).await.unwrap();
    assert_eq!(empty_obj_res.status(), StatusCode::BAD_REQUEST);
    let empty_obj_body: serde_json::Value = serde_json::from_slice(
        &empty_obj_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(empty_obj_body["errorCode"], "MISSING_REQUIRED_FIELDS");

    let missing_name_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "email": "a@example.com", "password": "secret123" }).to_string(),
        ))
        .unwrap();
    let missing_name_res = app.clone().oneshot(missing_name_req).await.unwrap();
    assert_eq!(missing_name_res.status(), StatusCode::BAD_REQUEST);
    let missing_name_body: serde_json::Value = serde_json::from_slice(
        &missing_name_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(missing_name_body["errorCode"], "MISSING_FULL_NAME");

    // 10. Short password -> 400 INVALID_PASSWORD
    let short_pw_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "pw@example.com",
                "fullName": "Andi Wijaya",
                "password": "short"
            })
            .to_string(),
        ))
        .unwrap();
    let short_pw_res = app.clone().oneshot(short_pw_req).await.unwrap();
    assert_eq!(short_pw_res.status(), StatusCode::BAD_REQUEST);
    let short_pw_body: serde_json::Value = serde_json::from_slice(
        &short_pw_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(short_pw_body["errorCode"], "INVALID_PASSWORD");

    // 11. Wrong method -> 405 METHOD_NOT_ALLOWED JSON
    let method_req = Request::builder()
        .method("GET")
        .uri("/api/v1/register")
        .body(Body::empty())
        .unwrap();
    let method_res = app.clone().oneshot(method_req).await.unwrap();
    assert_eq!(method_res.status(), StatusCode::METHOD_NOT_ALLOWED);
    let method_body: serde_json::Value = serde_json::from_slice(
        &method_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(method_body["errorCode"], "METHOD_NOT_ALLOWED");

    // 12. Oversized body (> 16 KB) -> 413 PAYLOAD_TOO_LARGE JSON
    let large_body = format!(
        r#"{{"email":"big@example.com","fullName":"{}","password":"secret123"}}"#,
        "a".repeat(20 * 1024)
    );
    let large_req = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header("content-type", "application/json")
        .body(Body::from(large_body))
        .unwrap();
    let large_res = app.clone().oneshot(large_req).await.unwrap();
    assert_eq!(large_res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let large_json: serde_json::Value = serde_json::from_slice(
        &large_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(large_json["errorCode"], "PAYLOAD_TOO_LARGE");

    // 13. Login, Refresh Rotation, and Idempotent Logout lifecycle
    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": unique_email,
                        "password": "secret123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    let login_auth: AuthResponseDto = serde_json::from_slice(
        &login_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(login_auth.user.email, unique_email);

    let bad_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": unique_email,
                        "password": "wrongpassword"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_login.status(), StatusCode::UNAUTHORIZED);
    let bad_login_body: serde_json::Value = serde_json::from_slice(
        &bad_login.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(bad_login_body["errorCode"], "INVALID_CREDENTIALS");

    // Empty/whitespace refreshToken -> 400 MISSING_REFRESH_TOKEN
    let empty_refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/refresh")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "refreshToken": "   " }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_refresh.status(), StatusCode::BAD_REQUEST);
    let empty_refresh_body: serde_json::Value = serde_json::from_slice(
        &empty_refresh.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(empty_refresh_body["errorCode"], "MISSING_REFRESH_TOKEN");

    // Refresh rotation: new pair works, reusing old refreshToken -> 401 INVALID_REFRESH_TOKEN
    let refresh_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refreshToken": login_auth.refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_res.status(), StatusCode::OK);
    let rotated: TokenPairDto = serde_json::from_slice(
        &refresh_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_ne!(rotated.refresh_token, login_auth.refresh_token);

    let reuse_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refreshToken": login_auth.refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reuse_res.status(), StatusCode::UNAUTHORIZED);
    let reuse_body: serde_json::Value = serde_json::from_slice(
        &reuse_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(reuse_body["errorCode"], "INVALID_REFRESH_TOKEN");

    // Logout idempotently revokes the rotated refreshToken
    for _ in 0..2 {
        let logout_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/logout")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "refreshToken": rotated.refresh_token }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_res.status(), StatusCode::NO_CONTENT);
    }

    let after_logout_refresh = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refreshToken": rotated.refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after_logout_refresh.status(), StatusCode::UNAUTHORIZED);
    let after_logout_body: serde_json::Value = serde_json::from_slice(
        &after_logout_refresh
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(after_logout_body["errorCode"], "INVALID_REFRESH_TOKEN");
}
