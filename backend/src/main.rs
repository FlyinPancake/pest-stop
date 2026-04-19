use std::{env, net::SocketAddr};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

#[derive(Clone)]
struct AppState {
    feed_revision: &'static str,
    demo_stops: Vec<Stop>,
}

#[derive(Clone, Serialize)]
struct Stop {
    id: &'static str,
    name: &'static str,
    lat: f64,
    lon: f64,
}

#[derive(Clone, Serialize)]
struct Departure {
    route_short_name: &'static str,
    headsign: &'static str,
    scheduled_time: &'static str,
    minutes: u8,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    feed_revision: &'static str,
}

#[derive(Serialize)]
struct StopSearchResponse {
    query: String,
    stops: Vec<Stop>,
}

#[derive(Serialize)]
struct DeparturesResponse {
    stop: Stop,
    generated_at: &'static str,
    next_refresh_seconds: u16,
    cursor: Option<&'static str>,
    departures: Vec<Departure>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Serialize)]
struct NearbyStop {
    #[serde(flatten)]
    stop: Stop,
    distance_m: u32,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct NearbyParams {
    lat: f64,
    lon: f64,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct DepartureParams {
    limit: Option<usize>,
}

#[tokio::main]
async fn main() {
    init_tracing();

    let state = AppState {
        feed_revision: "bkk-demo-2026-04-19",
        demo_stops: vec![
            Stop {
                id: "F01111",
                name: "Deak Ferenc ter M",
                lat: 47.4979,
                lon: 19.0542,
            },
            Stop {
                id: "F01112",
                name: "Astoria M",
                lat: 47.4932,
                lon: 19.0585,
            },
        ],
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/stops/search", get(search_stops))
        .route("/api/v1/stops/nearby", get(nearby_stops))
        .route("/api/v1/stops/{stop_id}/departures", get(stop_departures))
        .with_state(state)
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

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        feed_revision: state.feed_revision,
    })
}

async fn search_stops(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Json<StopSearchResponse> {
    let query = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(5).max(1);
    let query_lower = query.to_lowercase();

    let mut stops = state.demo_stops.clone();
    if !query_lower.is_empty() {
        stops.retain(|stop| stop.name.to_lowercase().contains(&query_lower));
    }
    stops.truncate(limit);

    Json(StopSearchResponse { query, stops })
}

async fn nearby_stops(
    State(state): State<AppState>,
    Query(params): Query<NearbyParams>,
) -> Json<Vec<NearbyStop>> {
    let limit = params.limit.unwrap_or(5).clamp(1, 20);

    let mut stops: Vec<NearbyStop> = state
        .demo_stops
        .iter()
        .enumerate()
        .map(|(i, stop)| NearbyStop {
            stop: stop.clone(),
            distance_m: 150 + (i as u32) * 200,
        })
        .collect();

    stops.truncate(limit);
    Json(stops)
}

async fn stop_departures(
    State(state): State<AppState>,
    Path(stop_id): Path<String>,
    Query(params): Query<DepartureParams>,
) -> impl IntoResponse {
    let stop = state.demo_stops.iter().find(|s| s.id == stop_id);
    let Some(stop) = stop else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "unknown stop id",
            }),
        )
            .into_response();
    };

    let mut departures = match stop.id {
        "F01111" => vec![
            Departure {
                route_short_name: "47",
                headsign: "Varoshaz ter",
                scheduled_time: "12:04",
                minutes: 2,
            },
            Departure {
                route_short_name: "M2",
                headsign: "Ors vezer tere",
                scheduled_time: "12:06",
                minutes: 4,
            },
            Departure {
                route_short_name: "100E",
                headsign: "Liszt Ferenc Airport 2",
                scheduled_time: "12:11",
                minutes: 9,
            },
        ],
        "F01112" => vec![
            Departure {
                route_short_name: "M2",
                headsign: "Deli palyaudvar",
                scheduled_time: "12:03",
                minutes: 1,
            },
            Departure {
                route_short_name: "7",
                headsign: "Bosnyak ter",
                scheduled_time: "12:05",
                minutes: 3,
            },
        ],
        _ => vec![],
    };
    departures.truncate(params.limit.unwrap_or(3).clamp(1, 10));

    (
        StatusCode::OK,
        Json(DeparturesResponse {
            stop: stop.clone(),
            generated_at: "2026-04-19T12:02:00Z",
            next_refresh_seconds: 30,
            cursor: None,
            departures,
        }),
    )
        .into_response()
}
