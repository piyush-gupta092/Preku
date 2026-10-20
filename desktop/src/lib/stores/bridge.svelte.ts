// The phone bridge: paired phones, their status, and the pairing QR while it is open.

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { BridgeState } from "../types/generated/BridgeState";

class BridgeStore {
  state = $state<BridgeState | null>(null);
  error = $state<string | null>(null);

  /** The first paired phone (the UI shows one; more can be paired). */
  phone = $derived(this.state?.phones[0] ?? null);

  async connect(): Promise<() => void> {
    const result = await ipc.bridgeGet();
    if (result.ok) this.state = result.value;
    return listen<BridgeState>("bridge:state", (e) => (this.state = e.payload));
  }

  async run(command: ReturnType<typeof ipc.bridgeGet>) {
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

export const bridge = new BridgeStore();
