use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use nexus_api::{router, AppState};
use std::sync::Arc;
use tower::ServiceExt;
const KEY: &str = "test-key-with-at-least-32-characters";
fn app() -> axum::Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unreachable")
        .unwrap();
    router(AppState {
        pool,
        api_key: Arc::from(KEY),
    })
}
#[tokio::test]
async fn liveness_does_not_require_database() {
    let r = app()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}
#[tokio::test]
async fn missing_key_is_unauthorized() {
    let r = app()
        .oneshot(
            Request::builder()
                .uri("/v1/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn wrong_key_is_unauthorized() {
    let r = app()
        .oneshot(
            Request::builder()
                .uri("/v1/stats")
                .header("x-api-key", "wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn invalid_batch_never_reaches_database() {
    let r = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/events")
                .header("x-api-key", KEY)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"events":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn malformed_json_rejected() {
    let r = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/events")
                .header("x-api-key", KEY)
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn oversized_body_rejected() {
    let r = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/events")
                .header("x-api-key", KEY)
                .header("content-type", "application/json")
                .body(Body::from("x".repeat(1_048_577)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
#[tokio::test]
async fn bad_limit_rejected() {
    let r = app()
        .oneshot(
            Request::builder()
                .uri("/v1/events?limit=101")
                .header("x-api-key", KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn openapi_has_auth_and_paths() {
    use http_body_util::BodyExt;
    let r = app()
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(doc["paths"]["/v1/events"]["post"].is_object());
    assert_eq!(
        doc["components"]["securitySchemes"]["api_key"]["name"],
        "X-API-Key"
    );
}
#[tokio::test]
async fn dashboard_is_served() {
    let r = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["x-content-type-options"], "nosniff");
}
