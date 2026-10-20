// System & Dev Monitor state: loaded once, then kept fresh by `monitor:*` events.

import { listen } from "@tauri-apps/api/event";
import { ipc } from "../ipc";
import type { AlertRule } from "../types/generated/AlertRule";
import type { CiStatus } from "../types/generated/CiStatus";
import type { DockerState } from "../types/generated/DockerState";
import type { MetricsSample } from "../types/generated/MetricsSample";
import type { ProcessInfo } from "../types/generated/ProcessInfo";
import type { UptimeStatus } from "../types/generated/UptimeStatus";
import type { WatchRecord } from "../types/generated/WatchRecord";
import type { WatchTarget } from "../types/generated/WatchTarget";

/** Same length as the backend history (metrics.rs HISTORY_LEN). */
const HISTORY_LEN = 60;

export const monitorEvents = {
  metrics: "monitor:metrics",
  processes: "monitor:processes",
  rules: "monitor:rules",
  docker: "monitor:docker",
  ci: "monitor:ci",
  uptime: "monitor:uptime",
  watches: "monitor:watches",
  targets: "monitor:targets",
} as const;

class MonitorStore {
  history = $state<MetricsSample[]>([]);
  /** The last hour, one averaged point per minute. */
  historyHour = $state<MetricsSample[]>([]);
  alertRepeatMin = $state(0);
  processes = $state<ProcessInfo[]>([]);
  rules = $state<AlertRule[]>([]);
  docker = $state<DockerState>({ available: false, containers: [] });
  ci = $state<CiStatus[]>([]);
  uptime = $state<UptimeStatus[]>([]);
  watches = $state<WatchRecord[]>([]);
  targets = $state<WatchTarget[]>([]);
  githubTokenSet = $state(false);
  error = $state<string | null>(null);

  latest = $derived<MetricsSample | undefined>(this.history[this.history.length - 1]);

  async load() {
    const result = await ipc.monitorGet();
    if (!result.ok) {
      this.error = result.error.message;
      return;
    }
    const s = result.value;
    this.history = s.history;
    this.historyHour = s.historyHour;
    this.alertRepeatMin = s.alertRepeatMin;
    this.processes = s.processes;
    this.rules = s.rules;
    this.docker = s.docker;
    this.ci = s.ci;
    this.uptime = s.uptime;
    this.watches = s.watches;
    this.targets = s.targets;
    this.githubTokenSet = s.githubTokenSet;
    this.error = null;
  }

  async connect(): Promise<() => void> {
    await this.load();
    const offs = await Promise.all([
      listen<MetricsSample>(monitorEvents.metrics, (e) => {
        this.history = [...this.history, e.payload].slice(-HISTORY_LEN);
        this.error = null;
      }),
      listen<MetricsSample[]>("monitor:history-hour", (e) => (this.historyHour = e.payload)),
      listen<ProcessInfo[]>(monitorEvents.processes, (e) => (this.processes = e.payload)),
      listen<AlertRule[]>(monitorEvents.rules, (e) => (this.rules = e.payload)),
      listen<DockerState>(monitorEvents.docker, (e) => (this.docker = e.payload)),
      listen<CiStatus[]>(monitorEvents.ci, (e) => (this.ci = e.payload)),
      listen<UptimeStatus[]>(monitorEvents.uptime, (e) => (this.uptime = e.payload)),
      listen<WatchRecord[]>(monitorEvents.watches, (e) => (this.watches = e.payload)),
      listen<WatchTarget[]>(monitorEvents.targets, (e) => (this.targets = e.payload)),
    ]);
    return () => offs.forEach((off) => off());
  }

  async run<T>(
    command: Promise<{ ok: true; value: T } | { ok: false; error: { message: string } }>,
  ) {
    const result = await command;
    this.error = result.ok ? null : result.error.message;
    return result;
  }
}

export const monitor = new MonitorStore();
