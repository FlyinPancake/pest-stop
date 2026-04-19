use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::api::{
    AppState,
    models::{Departure, Stop},
};

#[derive(OpenApi)]
#[openapi(info(title = "Pest Stop Backend"))]
pub struct ApiDoc;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    status: String,
    feed_revision: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Backend health and feed revision.", body = HealthResponse)
    )
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        feed_revision: state.feed_revision,
    })
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchParams {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct StopSearchResponse {
    query: String,
    stops: Vec<Stop>,
}

#[utoipa::path(
    get,
    path = "/api/v1/stops/search",
    params(SearchParams),
    responses(
        (status = 200, description = "Stops matching the search query.", body = StopSearchResponse)
    )
)]
pub async fn search_stops(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Json<StopSearchResponse> {
    let query = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(5).max(1);
    let stops = state.gtfs.search_stops(&query, limit);

    Json(StopSearchResponse { query, stops })
}

#[derive(Deserialize, IntoParams, Debug)]
#[into_params(parameter_in = Query)]
pub struct NearbyParams {
    lat: f64,
    lon: f64,
    limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct NearbyStop {
    #[serde(flatten)]
    #[schema(inline)]
    stop: Stop,
    distance_m: u32,
}

#[utoipa::path(
    get,
    path = "/api/v1/stops/nearby",
    params(NearbyParams),
    responses(
        (status = 200, description = "Nearby stops ordered by distance.", body = [NearbyStop])
    )
)]
#[tracing::instrument(skip(state))]
pub async fn nearby_stops(
    State(state): State<AppState>,
    Query(params): Query<NearbyParams>,
) -> Json<Vec<NearbyStop>> {
    let limit = params.limit.unwrap_or(5).clamp(1, 20);
    let stops = state
        .gtfs
        .nearby_stops(params.lat, params.lon, limit)
        .into_iter()
        .map(|(stop, distance_m)| NearbyStop { stop, distance_m })
        .collect();

    Json(stops)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DepartureParams {
    limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
struct DeparturesResponse {
    stop: Stop,
    generated_at: String,
    next_refresh_seconds: u16,
    cursor: Option<String>,
    departures: Vec<Departure>,
}

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    error: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/stops/{stop_id}/departures",
    params(
        ("stop_id" = String, Path, description = "Stop identifier."),
        DepartureParams
    ),
    responses(
        (status = 200, description = "Upcoming departures for the stop.", body = DeparturesResponse),
        (status = 404, description = "Stop was not found.", body = ErrorResponse)
    )
)]
pub async fn stop_departures(
    State(state): State<AppState>,
    Path(stop_id): Path<String>,
    Query(params): Query<DepartureParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(3).clamp(1, 10);
    let Some((stop, departures)) = state.gtfs.stop_departures(&stop_id, limit) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "unknown stop id".to_string(),
            }),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        Json(DeparturesResponse {
            stop,
            generated_at: Local::now().to_rfc3339(),
            next_refresh_seconds: 30,
            cursor: None,
            departures,
        }),
    )
        .into_response()
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .routes(routes!(search_stops))
        .routes(routes!(nearby_stops))
        .routes(routes!(stop_departures))
}
