// Pip's look (body style + color) from Preferences. Every Pip reads this, so a change in
// Settings shows up in the island and Settings at once. Connects itself on first use.

import { listen } from "@tauri-apps/api/event";
import { DEFAULT_MASCOT_COLOR } from "../../pip/palette";
import { events, ipc } from "../ipc";
import type { MascotStyle } from "../types/generated/MascotStyle";
import type { Preferences } from "../types/generated/Preferences";

class MascotStore {
  style = $state<MascotStyle>("classic");
  color = $state(DEFAULT_MASCOT_COLOR);
  private connected = false;

  apply(preferences: Pick<Preferences, "mascotStyle" | "mascotColor">) {
    this.style = preferences.mascotStyle ?? "classic";
    this.color = preferences.mascotColor ?? DEFAULT_MASCOT_COLOR;
  }

  /** Loads the saved look once and follows later changes; safe to call from every Pip. */
  ensureConnected() {
    if (this.connected) return;
    this.connected = true;
    void (async () => {
      try {
        const result = await ipc.preferencesGet();
        if (result.ok) this.apply(result.value);
        await listen<Preferences>(events.preferences, (e) => this.apply(e.payload));
      } catch {
        // No backend (tests): keep the defaults.
      }
    })();
  }
}

export const mascot = new MascotStore();
