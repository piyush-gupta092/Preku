// Wellbeing state: one snapshot from the backend, replaced on every `wellbeing:state` event.

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { WellbeingState } from "../types/generated/WellbeingState";

export const wellbeingEvents = { state: "wellbeing:state" } as const;

class WellbeingStore {
  state = $state<WellbeingState | null>(null);
  error = $state<string | null>(null);

  /** Fullscreen app or presentation running: the island shouldn't pop out. */
  dnd = $derived(this.state?.dnd ?? false);

  async connect(): Promise<() => void> {
    const result = await ipc.wellbeingGet();
    if (result.ok) this.state = result.value;
    else this.error = result.error.message;
    return listen<WellbeingState>(wellbeingEvents.state, (e) => {
      this.state = e.payload;
      this.error = null;
    });
  }

  /** Runs a command that returns the new state. */
  async run(
    command: Promise<
      { ok: true; value: WellbeingState } | { ok: false; error: { message: string } }
    >,
  ) {
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

export const wellbeing = new WellbeingStore();
