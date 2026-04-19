use std::{env, net::SocketAddr, sync::Arc};

use axum::{Json, Router, routing::get};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use utoipa::openapi::OpenApi;

mod api;
mod datasource;

#[tokio::main]
async fn main() -> Result<(), datasource::DynError> {
    init_tracing();

    let state = tokio::task::spawn_blocking(api::AppState::load).await??;

    let (router, openapi) = api::router().split_for_parts();
    let openapi = Arc::new(openapi);

    let docs = Router::new()
        .route("/api-docs/openapi.json", get(openapi_json))
        .with_state(AppStateWithDocs { openapi });

    let app = router
        .with_state(state)
        .merge(docs)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = listen_addr();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind listener");

    info!("listening on http://{}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    Ok(())
}

#[derive(Clone)]
struct AppStateWithDocs {
    openapi: Arc<OpenApi>,
}

async fn openapi_json(
    axum::extract::State(state): axum::extract::State<AppStateWithDocs>,
) -> Json<OpenApi> {
    Json((*state.openapi).clone())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bkk_server=debug,tower_http=info".into()),
        )
        .init();
}

fn listen_addr() -> SocketAddr {
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(3000);

    format!("{bind_addr}:{port}")
        .parse()
        .expect("invalid BIND_ADDR/PORT combination")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
