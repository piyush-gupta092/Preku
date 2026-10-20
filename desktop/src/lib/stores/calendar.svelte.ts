// The optional calendar (PRD A5): today's agenda and the next event from the calendars the
// user chose to add. Empty and silent until they add one (Settings → Modules → Productivity).

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { CalendarState } from "../types/generated/CalendarState";

class CalendarStore {
  state = $state<CalendarState | null>(null);
  error = $state<string | null>(null);

  /** At least one calendar is added and switched on. */
  active = $derived(this.state?.settings.sources.some((source) => source.enabled) ?? false);

  async connect(): Promise<() => void> {
    const result = await ipc.calendarGet();
    if (result.ok) this.state = result.value;
    return listen<CalendarState>("productivity:calendar", (e) => (this.state = e.payload));
  }

  async run(command: ReturnType<typeof ipc.calendarGet>) {
    const result = await command;
    if (result.ok) {
      this.state = result.value;
      this.error = null;
    } else {
      this.error = result.error.message;
    }
    return result;
  }
}

export const calendar = new CalendarStore();
