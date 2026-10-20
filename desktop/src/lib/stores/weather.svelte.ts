// Current weather for the user's chosen place (null until one is picked in Settings).

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { WeatherState } from "../types/generated/WeatherState";

class WeatherStore {
  state = $state<WeatherState | null>(null);

  async connect(): Promise<() => void> {
    const result = await ipc.weatherGet();
    if (result.ok) this.state = result.value;
    return listen<WeatherState | null>("weather:state", (e) => (this.state = e.payload));
  }
}

export const weather = new WeatherStore();
