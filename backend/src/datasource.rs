use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use chrono::{Local, Timelike};
use gtfs_bin::{
    GTFS_BIN_VERSION,
    compiler::Compiler,
    consumer::Consumer,
    models::{Date, Stop as GtfsStop, Time},
};
use memmap2::{Mmap, MmapOptions};
use tracing::info;

use crate::api::models::{Departure, Stop};

const DEFAULT_GTFS_SOURCE_URL: &str =
    "https://go.bkk.hu/api/static/v1/public-gtfs/budapest_gtfs.zip";

pub type DynError = Box<dyn std::error::Error + Send + Sync>;

pub struct GtfsData {
    _mmap: &'static Mmap,
    consumer: Consumer<'static>,
}

impl GtfsData {
    pub fn search_stops(&self, query: &str, limit: usize) -> Vec<Stop> {
        let query = fold_search_text(query);

        self.consumer
            .stops
            .iter()
            .filter_map(|stop| self.stop_response(stop))
            .filter(|stop| {
                query.is_empty()
                    || fold_search_text(&stop.id).contains(&query)
                    || fold_search_text(&stop.name).contains(&query)
            })
            .take(limit)
            .collect()
    }

    pub fn nearby_stops(&self, lat: f64, lon: f64, limit: usize) -> Vec<(Stop, u32)> {
        let mut stops: Vec<(Stop, u32)> = self
            .consumer
            .stops
            .iter()
            .filter_map(|stop| {
                let stop = self.stop_response(stop)?;
                let distance_m = haversine_meters(lat, lon, stop.lat, stop.lon);
                Some((stop, distance_m))
            })
            .collect();

        stops.sort_by_key(|(_, distance_m)| *distance_m);
        stops.truncate(limit);
        stops
    }

    pub fn stop_departures(&self, stop_id: &str, limit: usize) -> Option<(Stop, Vec<Departure>)> {
        let gtfs_stop = self.consumer.stop_by_id(stop_id)?;
        let stop = self.stop_response(gtfs_stop)?;

        let now = Local::now();
        let now_seconds = now.time().num_seconds_from_midnight();
        let service_date = Date::from(now.date_naive());
        let mut departures = self.departures_for_stop(gtfs_stop, service_date, now_seconds);
        departures.truncate(limit);

        Some((stop, departures))
    }

    fn stop_response(&self, stop: &GtfsStop) -> Option<Stop> {
        let coordinate = stop.coordinate.get()?;
        let id = self.consumer.stop_id(stop.id).to_string();
        let name = stop
            .name
            .get()
            .map(|name| self.consumer.string(name).to_string())
            .unwrap_or_else(|| id.clone());

        Some(Stop {
            id,
            name,
            lat: coordinate.lat_f64(),
            lon: coordinate.lon_f64(),
        })
    }

    fn departures_for_stop(
        &self,
        stop: &GtfsStop,
        service_date: Date,
        now_seconds: u32,
    ) -> Vec<Departure> {
        let mut departures: Vec<(u32, Departure)> = self
            .consumer
            .iter_trips_by_stop(stop.idx)
            .filter(|trip| {
                self.consumer
                    .is_service_active(trip.service_idx, service_date)
            })
            .filter_map(|trip| {
                let stop_time = self
                    .consumer
                    .stop_times_by_trip(trip.idx)
                    .iter()
                    .find(|stop_time| stop_time.stop_idx == stop.idx)?;
                let time = stop_time
                    .departure_time
                    .get()
                    .or_else(|| stop_time.arrival_time.get())?;

                if time.0 < now_seconds {
                    return None;
                }

                let route = self.consumer.route(trip.route_idx);
                let route_id = self.consumer.route_id(route.id);
                let route_short_name = route
                    .short_name
                    .get()
                    .or_else(|| route.long_name.get())
                    .map(|name| self.consumer.string(name).to_string())
                    .unwrap_or_else(|| self.consumer.route_id(route.id).to_string());
                let mode = route_mode_name(route_id, &route_short_name).to_string();
                let headsign = stop_time
                    .headsign
                    .get()
                    .or_else(|| trip.headsign.get())
                    .map(|headsign| self.consumer.string(headsign).to_string())
                    .unwrap_or_default();
                let minutes = ((time.0 - now_seconds).saturating_add(59) / 60).min(u8::MAX as u32);

                Some((
                    time.0,
                    Departure {
                        mode,
                        route_short_name,
                        headsign,
                        scheduled_time: format_gtfs_time(time),
                        minutes: minutes as u8,
                    },
                ))
            })
            .collect();

        departures.sort_by_key(|(time, _)| *time);
        departures
            .into_iter()
            .map(|(_, departure)| departure)
            .collect()
    }
}

fn route_mode_name(route_id: &str, route_short_name: &str) -> &'static str {
    if route_id.starts_with('H') || route_short_name.starts_with('H') {
        return "suburban-railway";
    }

    if let Ok(numeric_route_id) = route_id.parse::<u32>() {
        if (4700..4900).contains(&numeric_route_id) {
            return "trolleybus";
        }
        if (5100..5500).contains(&numeric_route_id) {
            return "subway";
        }
        if (3000..4000).contains(&numeric_route_id) {
            return "tram";
        }
        return "bus";
    }

    if route_short_name.starts_with('M') {
        return "subway";
    }

    "bus"
}

pub fn load() -> Result<(String, Arc<GtfsData>), DynError> {
    let paths = GtfsPaths::from_env();
    fs::create_dir_all(&paths.cache_dir)?;

    if !paths.zip_path.exists() {
        download_gtfs_zip(&paths.source_url, &paths.zip_path)?;
    }

    if should_compile_gtfs(&paths.zip_path, &paths.bin_path)? {
        compile_gtfs_binary(&paths.zip_path, &paths.bin_path)?;
    }

    let file = File::open(&paths.bin_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let mmap = Box::leak(Box::new(mmap));
    let consumer = Consumer::new(mmap)?;
    let feed_revision = feed_revision(&paths)?;

    info!(
        source_url = %paths.source_url,
        zip_path = %paths.zip_path.display(),
        bin_path = %paths.bin_path.display(),
        stops = consumer.stops.len(),
        routes = consumer.routes.len(),
        trips = consumer.trips.len(),
        "loaded gtfs datasource"
    );

    Ok((
        feed_revision,
        Arc::new(GtfsData {
            _mmap: mmap,
            consumer,
        }),
    ))
}

struct GtfsPaths {
    source_url: String,
    cache_dir: PathBuf,
    zip_path: PathBuf,
    bin_path: PathBuf,
}

impl GtfsPaths {
    fn from_env() -> Self {
        let source_url =
            env::var("GTFS_SOURCE_URL").unwrap_or_else(|_| DEFAULT_GTFS_SOURCE_URL.to_string());
        let cache_dir = env::var_os("GTFS_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"));
        let zip_path = env::var_os("GTFS_ZIP_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| cache_dir.join("budapest_gtfs.zip"));
        let bin_path = env::var_os("GTFS_BIN_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| cache_dir.join("budapest.gtfs"));

        Self {
            source_url,
            cache_dir,
            zip_path,
            bin_path,
        }
    }
}

fn download_gtfs_zip(source_url: &str, zip_path: &Path) -> Result<(), DynError> {
    info!(source_url, zip_path = %zip_path.display(), "downloading gtfs feed");

    if let Some(parent) = zip_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = zip_path.with_extension("zip.tmp");
    let mut response = reqwest::blocking::get(source_url)?.error_for_status()?;
    let mut file = File::create(&tmp_path)?;
    response.copy_to(&mut file)?;
    file.flush()?;
    fs::rename(tmp_path, zip_path)?;

    Ok(())
}

fn should_compile_gtfs(zip_path: &Path, bin_path: &Path) -> Result<bool, DynError> {
    if !bin_path.exists() {
        return Ok(true);
    }

    let zip_modified = fs::metadata(zip_path)?.modified()?;
    let bin_modified = fs::metadata(bin_path)?.modified()?;
    Ok(zip_modified > bin_modified)
}

fn compile_gtfs_binary(zip_path: &Path, bin_path: &Path) -> Result<(), DynError> {
    info!(zip_path = %zip_path.display(), bin_path = %bin_path.display(), "compiling gtfs feed");

    if let Some(parent) = bin_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = Compiler::new(zip_path).compile()?;
    let tmp_path = bin_path.with_extension("gtfs.tmp");
    let mut file = File::create(&tmp_path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    fs::rename(tmp_path, bin_path)?;

    Ok(())
}

fn feed_revision(paths: &GtfsPaths) -> Result<String, DynError> {
    let zip_metadata = fs::metadata(&paths.zip_path)?;
    let bin_metadata = fs::metadata(&paths.bin_path)?;
    let zip_modified = zip_metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let bin_modified = bin_metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    Ok(format!(
        "gtfs-bin-v{GTFS_BIN_VERSION}:zip-{}-{}:bin-{}",
        zip_metadata.len(),
        zip_modified,
        bin_modified
    ))
}

fn format_gtfs_time(time: Time) -> String {
    let hours = time.0 / 3600;
    let minutes = (time.0 % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

fn haversine_meters(origin_lat: f64, origin_lon: f64, target_lat: f64, target_lon: f64) -> u32 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;

    let origin_lat = origin_lat.to_radians();
    let target_lat = target_lat.to_radians();
    let delta_lat = target_lat - origin_lat;
    let delta_lon = (target_lon - origin_lon).to_radians();
    let a = (delta_lat / 2.0).sin().powi(2)
        + origin_lat.cos() * target_lat.cos() * (delta_lon / 2.0).sin().powi(2);
    let distance = 2.0 * EARTH_RADIUS_M * a.sqrt().asin();

    distance.round().clamp(0.0, u32::MAX as f64) as u32
}

fn fold_search_text(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            'á' | 'Á' => folded.push('a'),
            'é' | 'É' => folded.push('e'),
            'í' | 'Í' => folded.push('i'),
            'ó' | 'Ó' | 'ö' | 'Ö' | 'ő' | 'Ő' => folded.push('o'),
            'ú' | 'Ú' | 'ü' | 'Ü' | 'ű' | 'Ű' => folded.push('u'),
            _ => folded.extend(ch.to_lowercase()),
        }
    }

    folded
}
