//! Weather for a place the user picks (opt-in). Uses Open-Meteo: free, no account or key.
//! Only the searched city name and then the chosen place's coordinates leave this PC — never
//! an IP- or GPS-based location (Rules.md §4).

use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::sync::Notify;
use ts_rs::TS;

use super::{Module, ModuleContext};
use crate::foundation::{
    errors::{PrekuError, Result},
    preferences::{self, TemperatureUnit, WeatherPlace},
    storage::now_millis,
};

pub const ID: &str = "weather";
pub const STATE_EVENT: &str = "weather:state";
const REFRESH_EVERY: Duration = Duration::from_secs(30 * 60);
const MAX_RESULTS: usize = 6;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WeatherState {
    pub place: String,
    pub temperature: f32,
    pub high: f32,
    pub low: f32,
    /// WMO weather code (0 clear … 99 thunderstorm); the UI maps it to an icon and words.
    pub code: u16,
    pub is_day: bool,
    pub unit: TemperatureUnit,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f32,
    weather_code: u16,
    is_day: u8,
}

#[derive(Deserialize)]
struct Daily {
    temperature_2m_max: Vec<f32>,
    temperature_2m_min: Vec<f32>,
}

/// Turns an Open-Meteo forecast response into the UI state. Pure, so it's unit-tested.
pub fn parse_forecast(
    body: &str,
    place: &WeatherPlace,
    unit: TemperatureUnit,
    now: i64,
) -> Result<WeatherState> {
    let parsed: ForecastResponse = serde_json::from_str(body)?;
    let first = |values: &[f32]| {
        values
            .first()
            .copied()
            .unwrap_or(parsed.current.temperature_2m)
    };
    Ok(WeatherState {
        place: place.name.clone(),
        temperature: parsed.current.temperature_2m,
        high: first(&parsed.daily.temperature_2m_max),
        low: first(&parsed.daily.temperature_2m_min),
        code: parsed.current.weather_code,
        is_day: parsed.current.is_day == 1,
        unit,
        updated_at: now,
    })
}

#[derive(Deserialize)]
struct GeocodeResponse {
    #[serde(default)]
    results: Vec<GeocodeResult>,
}

#[derive(Deserialize)]
struct GeocodeResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

/// Parses Open-Meteo geocoding results into places the user can pick. Pure.
pub fn parse_places(body: &str) -> Result<Vec<WeatherPlace>> {
    let parsed: GeocodeResponse = serde_json::from_str(body)?;
    Ok(parsed
        .results
        .into_iter()
        .take(MAX_RESULTS)
        .map(|r| WeatherPlace {
            detail: [r.admin1, r.country]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty() && *part != r.name)
                .collect::<Vec<_>>()
                .join(", "),
            name: r.name,
            latitude: r.latitude,
            longitude: r.longitude,
        })
        .collect())
}

fn http_err(err: reqwest::Error) -> PrekuError {
    PrekuError::Io(std::io::Error::other(err.without_url()))
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Preku/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(http_err)
}

async fn fetch(
    client: &reqwest::Client,
    place: &WeatherPlace,
    unit: TemperatureUnit,
) -> Result<WeatherState> {
    let unit_param = match unit {
        TemperatureUnit::Celsius => "celsius",
        TemperatureUnit::Fahrenheit => "fahrenheit",
    };
    let body = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", place.latitude.to_string()),
            ("longitude", place.longitude.to_string()),
            ("current", "temperature_2m,weather_code,is_day".into()),
            ("daily", "temperature_2m_max,temperature_2m_min".into()),
            ("timezone", "auto".into()),
            ("forecast_days", "1".into()),
            ("temperature_unit", unit_param.into()),
        ])
        .send()
        .await
        .map_err(http_err)?
        .error_for_status()
        .map_err(http_err)?
        .text()
        .await
        .map_err(http_err)?;
    parse_forecast(&body, place, unit, now_millis())
}

/// City search for the Settings picker.
pub async fn search(query: &str) -> Result<Vec<WeatherPlace>> {
    let query = query.trim();
    if query.chars().count() < 2 || query.len() > 100 {
        return Ok(Vec::new());
    }
    let body = client()?
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[
            ("name", query),
            ("count", "6"),
            ("language", "en"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(http_err)?
        .error_for_status()
        .map_err(http_err)?
        .text()
        .await
        .map_err(http_err)?;
    parse_places(&body)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Shared {
    ctx: ModuleContext,
    state: Mutex<Option<WeatherState>>,
    wake: Notify,
}

#[derive(Default)]
pub struct WeatherModule {
    running: Mutex<Option<(Arc<Shared>, JoinHandle<()>)>>,
}

impl Module for WeatherModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let shared = Arc::new(Shared {
            ctx,
            state: Mutex::new(None),
            wake: Notify::new(),
        });
        let task = tauri::async_runtime::spawn(refresh_loop(shared.clone()));
        *lock(&self.running) = Some((shared, task));
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some((shared, task)) = lock(&self.running).take() {
            task.abort();
            shared.ctx.emit(STATE_EVENT, None::<WeatherState>);
        }
        Ok(())
    }
}

async fn refresh_loop(shared: Arc<Shared>) {
    let client = match client() {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(error = %err, "weather unavailable");
            return;
        }
    };
    loop {
        // Read the place each time: the user may have changed it in Settings.
        let prefs = preferences::load(&shared.ctx.storage).unwrap_or_default();
        let next = match prefs.weather_place {
            Some(place) => match fetch(&client, &place, prefs.temperature_unit).await {
                Ok(state) => Some(state),
                Err(err) => {
                    // Keep showing the last reading when offline.
                    tracing::warn!(error = %err, "weather refresh failed");
                    lock(&shared.state).clone()
                }
            },
            None => None,
        };
        *lock(&shared.state) = next.clone();
        shared.ctx.emit(STATE_EVENT, &next);
        tokio::select! {
            _ = tokio::time::sleep(REFRESH_EVERY) => {}
            _ = shared.wake.notified() => {}
        }
    }
}

impl WeatherModule {
    fn shared(&self) -> Option<Arc<Shared>> {
        lock(&self.running)
            .as_ref()
            .map(|(shared, _)| shared.clone())
    }

    pub fn current(&self) -> Option<WeatherState> {
        self.shared().and_then(|shared| lock(&shared.state).clone())
    }

    /// Fetch again now (after the place or unit changed).
    pub fn refresh(&self) {
        if let Some(shared) = self.shared() {
            shared.wake.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn london() -> WeatherPlace {
        WeatherPlace {
            name: "London".into(),
            detail: "England, United Kingdom".into(),
            latitude: 51.5,
            longitude: -0.12,
        }
    }

    #[test]
    fn parses_a_forecast() {
        let body = r#"{
            "current": {"temperature_2m": 17.4, "weather_code": 3, "is_day": 1},
            "daily": {"temperature_2m_max": [19.2], "temperature_2m_min": [11.0]}
        }"#;
        let state = parse_forecast(body, &london(), TemperatureUnit::Celsius, 42).unwrap();
        assert_eq!(state.place, "London");
        assert_eq!(
            (state.temperature, state.high, state.low),
            (17.4, 19.2, 11.0)
        );
        assert_eq!(state.code, 3);
        assert!(state.is_day);
        assert_eq!(state.updated_at, 42);
    }

    #[test]
    fn missing_daily_values_fall_back_to_current() {
        let body = r#"{"current": {"temperature_2m": 5.0, "weather_code": 71, "is_day": 0},
                       "daily": {"temperature_2m_max": [], "temperature_2m_min": []}}"#;
        let state = parse_forecast(body, &london(), TemperatureUnit::Fahrenheit, 0).unwrap();
        assert_eq!((state.high, state.low), (5.0, 5.0));
        assert!(!state.is_day);
        assert!(parse_forecast("{}", &london(), TemperatureUnit::Celsius, 0).is_err());
    }

    #[test]
    fn parses_places_with_readable_detail() {
        let body = r#"{"results": [
            {"name": "Paris", "latitude": 48.85, "longitude": 2.35, "country": "France", "admin1": "Île-de-France"},
            {"name": "Paris", "latitude": 33.66, "longitude": -95.55, "country": "United States", "admin1": "Texas"},
            {"name": "Singapore", "latitude": 1.29, "longitude": 103.85, "country": "Singapore", "admin1": null}
        ]}"#;
        let places = parse_places(body).unwrap();
        assert_eq!(places.len(), 3);
        assert_eq!(places[0].detail, "Île-de-France, France");
        assert_eq!(places[1].detail, "Texas, United States");
        assert_eq!(
            places[2].detail, "",
            "country equal to the name is not repeated"
        );
        assert!(parse_places(r#"{}"#).unwrap().is_empty(), "no matches");
    }
}
