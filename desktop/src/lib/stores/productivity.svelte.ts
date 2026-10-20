// Productivity state shared by the island tabs: loaded once, then kept fresh by backend events.

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { ClipItem } from "../types/generated/ClipItem";
import type { NowPlaying } from "../types/generated/NowPlaying";
import type { Task } from "../types/generated/Task";
import type { TimerSnapshot } from "../types/generated/TimerSnapshot";
import type { TodaySummary } from "../types/generated/TodaySummary";

export const productivityEvents = {
  timer: "productivity:timer",
  timerFinished: "productivity:timer-finished",
  tasks: "productivity:tasks",
  clipboard: "productivity:clipboard",
  media: "productivity:media",
} as const;

const idleTimer: TimerSnapshot = {
  status: "idle",
  label: "",
  totalS: 0,
  remainingS: 0,
  endsAt: null,
  taskId: null,
  finished: null,
};

class ProductivityStore {
  timer = $state<TimerSnapshot>(idleTimer);
  tasks = $state<Task[]>([]);
  clipboard = $state<ClipItem[]>([]);
  media = $state<NowPlaying | null>(null);
  /** Today's recap: recorded focus time and finished tasks (counts, never a score). */
  today = $state<TodaySummary>({
    focusS: 0,
    sessions: 0,
    tasksDone: 0,
    priorities: 0,
    prioritiesDone: 0,
  });
  /** The length picked in the Focus tab, also used by "Focus on this" from a task. */
  minutes = $state(25);
  /** Set when the module is off or failed to load, shown instead of the tabs' content. */
  error = $state<string | null>(null);

  /** Loads everything and subscribes. Returns an unsubscribe function. */
  async connect(onTimerFinished: (label: string) => void): Promise<() => void> {
    const [timer, tasks, clipboard, media, today] = await Promise.all([
      ipc.timerGet(),
      ipc.tasksList(),
      ipc.clipboardList(),
      ipc.mediaGet(),
      ipc.todaySummary(),
    ]);
    if (today.ok) this.today = today.value;
    if (timer.ok) this.timer = timer.value;
    if (tasks.ok) this.tasks = tasks.value;
    if (clipboard.ok) this.clipboard = clipboard.value;
    if (media.ok) this.media = media.value;
    this.error = tasks.ok ? null : tasks.error.message;

    const offs = await Promise.all([
      listen<TimerSnapshot>(productivityEvents.timer, (e) => (this.timer = e.payload)),
      listen<string>(productivityEvents.timerFinished, (e) => onTimerFinished(e.payload)),
      listen<Task[]>(productivityEvents.tasks, (e) => {
        this.tasks = e.payload;
        this.error = null;
      }),
      listen<ClipItem[]>(productivityEvents.clipboard, (e) => (this.clipboard = e.payload)),
      listen<NowPlaying | null>(productivityEvents.media, (e) => (this.media = e.payload)),
      listen<TodaySummary>("productivity:today", (e) => (this.today = e.payload)),
    ]);
    return () => offs.forEach((off) => off());
  }

  /** Runs a command; a failure is shown to the user instead of being silently dropped. */
  async run<T>(
    command: Promise<{ ok: true; value: T } | { ok: false; error: { message: string } }>,
  ) {
    const result = await command;
    if (!result.ok) this.error = result.error.message;
    return result;
  }
}

export const productivity = new ProductivityStore();
