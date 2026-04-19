pub mod models;
pub mod routes;

use std::sync::Arc;

use crate::datasource::{self, DynError, GtfsData};

pub use routes::router;

#[derive(Clone)]
pub struct AppState {
    pub feed_revision: String,
    pub gtfs: Arc<GtfsData>,
}

impl AppState {
    pub fn load() -> Result<Self, DynError> {
        let (feed_revision, gtfs) = datasource::load()?;

        Ok(Self {
            feed_revision,
            gtfs,
        })
    }
}
