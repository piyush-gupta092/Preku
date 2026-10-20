// The wellbeing companion's flow: a prompt from the backend invites a break; "Take a break"
// starts a guided break here (timed in the frontend, the backend only hears the result);
// finishing it shows a short encouragement. "Later" snoozes in the backend.

import { ACTIVITIES } from "../companion";
import { ipc } from "../ipc";
import type { Prompt } from "../types/generated/Prompt";
import type { ReminderKind } from "../types/generated/ReminderKind";
import { wellbeing } from "./wellbeing.svelte";

export type CompanionPhase = "ask" | "break" | "done";

export interface CompanionSession {
  kind: ReminderKind;
  phase: CompanionPhase;
  /** When a timed break ends (ms), for the countdown. */
  endsAt: number | null;
}

const CHEER_MS = 4000;

class CompanionStore {
  session = $state<CompanionSession | null>(null);
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** The prompt waiting for an answer, if any. */
  get prompt(): Prompt | null {
    return wellbeing.state?.prompt ?? null;
  }

  /** Something for the companion card to show. */
  get active(): boolean {
    return this.session !== null || this.prompt !== null;
  }

  /** "Take a break" (or the user asking for a breather / check-in themselves). */
  async take(kind: ReminderKind) {
    clearTimeout(this.timer);
    if (kind === "moodCheckIn") {
      this.session = { kind, phase: "ask", endsAt: null };
      return;
    }
    if (kind === "water") {
      await wellbeing.run(ipc.waterAdd());
      await this.finish(kind, true);
      return;
    }
    const seconds = ACTIVITIES[kind].breakS;
    if (seconds === 0) {
      await this.finish(kind, true);
      return;
    }
    this.session = { kind, phase: "break", endsAt: Date.now() + seconds * 1000 };
    this.timer = setTimeout(() => void this.finish(kind, true), seconds * 1000);
  }

  async later(prompt: Prompt) {
    await wellbeing.run(ipc.wellbeingPromptLater(prompt.id));
  }

  /** End a break: `completed` = the user did it (counted, timers restart) or skipped it. */
  async finish(kind: ReminderKind, completed: boolean) {
    clearTimeout(this.timer);
    await wellbeing.run(ipc.wellbeingBreakDone(kind, completed));
    this.cheerOrClose(kind, completed);
  }

  async logMood(mood: string) {
    clearTimeout(this.timer);
    const result = await wellbeing.run(ipc.moodLog(mood));
    this.cheerOrClose("moodCheckIn", result.ok);
  }

  private cheerOrClose(kind: ReminderKind, completed: boolean) {
    if (completed && wellbeing.state?.settings.encouragementEnabled) {
      this.session = { kind, phase: "done", endsAt: null };
      this.timer = setTimeout(() => (this.session = null), CHEER_MS);
    } else {
      this.session = null;
    }
  }

  /** Close the card without answering (e.g. the ✕ on a check-in the user opened). */
  close() {
    clearTimeout(this.timer);
    this.session = null;
  }
}

export const companion = new CompanionStore();
