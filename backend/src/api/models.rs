use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Serialize, ToSchema)]
pub struct Stop {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct Departure {
    pub mode: String,
    pub route_short_name: String,
    pub headsign: String,
    pub scheduled_time: String,
    pub minutes: u8,
}
