// Quiet mode (usability U4): routine prompts wait while the user paused them, or while a
// focus session runs with "hold reminders while I focus" on.

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { QuietState } from "../types/generated/QuietState";

class QuietStore {
  state = $state<QuietState>({
    until: null,
    holdInFocus: true,
    holdFullscreenApps: false,
    focusing: false,
    screenBusy: false,
  });

  async connect(): Promise<() => void> {
    const result = await ipc.quietGet();
    if (result.ok) this.state = result.value;
    return listen<QuietState>("app:quiet", (e) => (this.state = e.payload));
  }

  /** Paused by the user until this time (ms), if that is still ahead. */
  pausedUntil(now: number): number | null {
    return this.state.until !== null && this.state.until > now ? this.state.until : null;
  }

  async pause(until: number | null) {
    const result = await ipc.quietPause(until);
    if (result.ok) this.state = result.value;
  }

  async holdFullscreenApps(hold: boolean) {
    const result = await ipc.quietHoldFullscreenApps(hold);
    if (result.ok) this.state = result.value;
  }

  async holdInFocus(hold: boolean) {
    const result = await ipc.quietHoldInFocus(hold);
    if (result.ok) this.state = result.value;
  }
}

export const quiet = new QuietStore();

/** The pause choices: minutes from now, or until tomorrow morning (8:00). */
export function pauseUntil(choice: 30 | 60 | "tomorrow", now: number): number {
  if (choice !== "tomorrow") return now + choice * 60_000;
  const morning = new Date(now);
  morning.setDate(morning.getDate() + 1);
  morning.setHours(8, 0, 0, 0);
  return morning.getTime();
}
