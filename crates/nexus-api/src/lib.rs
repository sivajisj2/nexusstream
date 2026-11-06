//! HTTP boundary: authenticated ingestion, explorer, operational endpoints and docs.
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use nexus_protocol::{Batch, Event, EventInput, Receipt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub api_key: Arc<str>,
}
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}
pub struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorBody { error: self.1 })).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error=%e,"database request failed");
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "database unavailable".into(),
        )
    }
}
impl From<nexus_store::StoreError> for ApiError {
    fn from(e: nexus_store::StoreError) -> Self {
        match e {
            nexus_store::StoreError::Conflict => Self(StatusCode::CONFLICT, e.to_string()),
            _ => {
                tracing::error!(error=%e,"ingestion failed");
                Self(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "ingestion unavailable".into(),
                )
            }
        }
    }
}

async fn auth(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let provided = req
        .headers()
        .get("x-api-key")
        .map(|h| h.as_bytes())
        .unwrap_or_default();
    if !bool::from(provided.ct_eq(s.api_key.as_bytes())) {
        return ApiError(StatusCode::UNAUTHORIZED, "valid X-API-Key required".into())
            .into_response();
    }
    next.run(req).await
}

#[utoipa::path(post,path="/v1/events",request_body=Batch,responses((status=202,body=Receipt),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=409,body=ErrorBody),(status=413,description="Body exceeds 1 MiB"),(status=503,body=ErrorBody)),security(("api_key"=[])),tag="Events")]
async fn ingest(
    State(s): State<AppState>,
    Json(batch): Json<Batch>,
) -> Result<(StatusCode, Json<Receipt>), ApiError> {
    nexus_protocol::validate(&batch)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(nexus_store::ingest(&s.pool, &batch).await?),
    ))
}
#[derive(Default, Deserialize, utoipa::IntoParams)]
pub struct Filters {
    pub before: Option<i64>,
    pub limit: Option<i64>,
    pub source: Option<String>,
    pub kind: Option<String>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Page {
    pub events: Vec<Event>,
    pub next_cursor: Option<i64>,
}
#[utoipa::path(get,path="/v1/events",params(Filters),responses((status=200,body=Page),(status=401,body=ErrorBody),(status=400,body=ErrorBody),(status=503,body=ErrorBody)),security(("api_key"=[])),tag="Events")]
async fn list(State(s): State<AppState>, Query(f): Query<Filters>) -> Result<Json<Page>, ApiError> {
    let limit = f.limit.unwrap_or(25);
    if !(1..=100).contains(&limit) || f.before.is_some_and(|v| v <= 0) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "limit must be 1..100; before must be positive".into(),
        ));
    }
    let mut rows = nexus_store::list(
        &s.pool,
        f.before.unwrap_or(i64::MAX),
        limit + 1,
        f.source.as_deref(),
        f.kind.as_deref(),
    )
    .await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if more {
        rows.last().map(|(seq, _)| *seq)
    } else {
        None
    };
    Ok(Json(Page {
        events: rows.into_iter().map(|(_, e)| e).collect(),
        next_cursor,
    }))
}
#[utoipa::path(get,path="/v1/events/{id}",params(("id"=Uuid,Path)),responses((status=200,body=Event),(status=404,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)),security(("api_key"=[])),tag="Events")]
async fn event(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Event>, ApiError> {
    nexus_store::get(&s.pool, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "event not found".into()))
}
#[utoipa::path(post,path="/v1/events/{id}/replay",params(("id"=Uuid,Path)),responses((status=204,description="Dead delivery queued"),(status=409,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)),security(("api_key"=[])),tag="Operations")]
async fn replay(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, ApiError> {
    if nexus_store::replay(&s.pool, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError(
            StatusCode::CONFLICT,
            "event does not have a dead delivery".into(),
        ))
    }
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Stats {
    pub total: i64,
    pub pending: i64,
    pub done: i64,
    pub dead: i64,
}
#[utoipa::path(get,path="/v1/stats",responses((status=200,body=Stats),(status=401,body=ErrorBody),(status=503,body=ErrorBody)),security(("api_key"=[])),tag="Operations")]
async fn stats(State(s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(nexus_store::stats(&s.pool).await?))
}
#[utoipa::path(get,path="/healthz",responses((status=200,description="Process alive")),tag="Health")]
async fn health() -> &'static str {
    "ok"
}
#[utoipa::path(get,path="/readyz",responses((status=200,description="Database ready"),(status=503,body=ErrorBody)),tag="Health")]
async fn ready(State(s): State<AppState>) -> Result<&'static str, ApiError> {
    sqlx::query("SELECT 1").execute(&s.pool).await?;
    Ok("ready")
}
async fn metrics(State(s): State<AppState>) -> Result<String, ApiError> {
    let stats = nexus_store::stats(&s.pool).await?;
    let mut out = String::new();
    for k in ["total", "pending", "done", "dead"] {
        out.push_str(&format!(
            "# TYPE nexus_events_{k} gauge\nnexus_events_{k} {}\n",
            stats[k].as_i64().unwrap_or(0)
        ));
    }
    Ok(out)
}
struct Security;
impl utoipa::Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(c) = openapi.components.as_mut() {
            c.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    ),
                ),
            );
        }
    }
}
#[derive(OpenApi)]
#[openapi(info(title="NexusStream API",version="1.0.0",description="Durable event ingestion with atomic batches and idempotent IDs."),paths(ingest,list,event,replay,stats,health,ready),components(schemas(Batch,EventInput,Event,Receipt,ErrorBody,Page,Stats)),modifiers(&Security))]
pub struct ApiDoc;

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/v1/events", get(list).post(ingest))
        .route("/v1/events/{id}", get(event))
        .route("/v1/events/{id}/replay", post(replay))
        .route("/v1/stats", get(stats))
        .route("/metrics", get(metrics))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth));
    Router::new()
        .merge(protected)
        .route(
            "/",
            get(|| async { Html(include_str!("../../../web/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/javascript")],
                    include_str!("../../../web/app.js"),
                )
            }),
        )
        .route(
            "/app.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../../../web/app.css"),
                )
            }),
        )
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/docs")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .layer(DefaultBodyLimit::max(1_048_576))
        .layer(
            tower_http::set_header::SetResponseHeaderLayer::if_not_present(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        )
        .layer(
            tower_http::set_header::SetResponseHeaderLayer::if_not_present(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store"),
            ),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
