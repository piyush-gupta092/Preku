// Installed skills (data-only add-ons). Both windows read them: the notch for thoughts and
// nudge sounds, Settings to install, switch and remove them.

import { listen } from "@tauri-apps/api/event";
import { events, ipc } from "../ipc";
import type { InstalledSkill } from "../types/generated/InstalledSkill";
import type { SkillsState } from "../types/generated/SkillsState";

class SkillsStore {
  state = $state<SkillsState>({ skills: [], broken: [] });
  error = $state<string | null>(null);

  installed = $derived<InstalledSkill[]>(this.state.skills);

  async connect(): Promise<() => void> {
    const result = await ipc.skillsGet();
    if (result.ok) this.state = result.value;
    return listen<SkillsState>(events.skills, (e) => (this.state = e.payload));
  }

  async run(command: ReturnType<typeof ipc.skillsGet>) {
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

export const skills = new SkillsStore();
