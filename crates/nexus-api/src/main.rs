use nexus_api::{router, AppState};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let api_key = std::env::var("NEXUS_API_KEY").map_err(|_| "NEXUS_API_KEY is required")?;
    if api_key.len() < 32 {
        return Err("NEXUS_API_KEY must contain at least 32 bytes".into());
    }
    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await?;
    nexus_store::migrate(&pool).await?;
    let state = AppState {
        pool: pool.clone(),
        api_key: Arc::from(api_key),
    };
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        loop {
            if *stop_rx.borrow() {
                break;
            }
            let delay = match nexus_store::process_one(&worker_pool).await {
                Ok(true) => Duration::from_millis(1),
                Ok(false) => Duration::from_millis(250),
                Err(e) => {
                    tracing::error!(error=%e,"worker failed; retrying");
                    Duration::from_secs(2)
                }
            };
            tokio::select! {_ = tokio::time::sleep(delay)=>{}, _ = stop_rx.changed()=>{break;}}
        }
    });
    let bind = std::env::var("NEXUS_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(address=%bind,"NexusStream ready");
    let result = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown())
        .await;
    let _ = stop_tx.send(true);
    worker.await?;
    pool.close().await;
    result?;
    Ok(())
}
async fn shutdown() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error=%e,"signal handler failed")
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {_ = ctrl_c=>{},_ = terminate=>{}}
    tracing::info!("draining HTTP requests");
}
