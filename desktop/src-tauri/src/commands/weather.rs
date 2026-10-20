//! Weather commands (async: they hit the network).

use std::sync::Arc;

use tauri::State;

use crate::{
    foundation::{errors::IpcError, preferences::WeatherPlace},
    modules::weather::{self, WeatherModule, WeatherState},
};

type Module<'a> = State<'a, Arc<WeatherModule>>;

#[tauri::command]
pub async fn weather_get(module: Module<'_>) -> Result<Option<WeatherState>, IpcError> {
    Ok(module.current())
}

/// City search for the Settings picker (Open-Meteo geocoding).
#[tauri::command]
pub async fn weather_search(query: String) -> Result<Vec<WeatherPlace>, IpcError> {
    Ok(weather::search(&query).await?)
}

/// Fetch again now, e.g. after the place or unit changed.
#[tauri::command]
pub async fn weather_refresh(module: Module<'_>) -> Result<(), IpcError> {
    module.refresh();
    Ok(())
}
