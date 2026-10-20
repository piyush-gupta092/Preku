import { invoke } from "@tauri-apps/api/core";
import type { AlertRule } from "./types/generated/AlertRule";
import type { AppInfo } from "./types/generated/AppInfo";
import type { ContainerAction } from "./types/generated/ContainerAction";
import type { MonitorSnapshot } from "./types/generated/MonitorSnapshot";
import type { MonitorView } from "./types/generated/MonitorView";
import type { ProcessInfo } from "./types/generated/ProcessInfo";
import type { TargetKind } from "./types/generated/TargetKind";
import type { WatchTarget } from "./types/generated/WatchTarget";
import type { WeatherPlace } from "./types/generated/WeatherPlace";
import type { WeatherState } from "./types/generated/WeatherState";
import type { WellbeingSettings } from "./types/generated/WellbeingSettings";
import type { ReminderKind } from "./types/generated/ReminderKind";
import type { BridgeState } from "./types/generated/BridgeState";
import type { CalendarState } from "./types/generated/CalendarState";
import type { PrivacySettings } from "./types/generated/PrivacySettings";
import type { QuietState } from "./types/generated/QuietState";
import type { TodaySummary } from "./types/generated/TodaySummary";
import type { WellbeingState } from "./types/generated/WellbeingState";
import type { ClipItem } from "./types/generated/ClipItem";
import type { MediaAction } from "./types/generated/MediaAction";
import type { NowPlaying } from "./types/generated/NowPlaying";
import type { Task } from "./types/generated/Task";
import type { TimerSnapshot } from "./types/generated/TimerSnapshot";
import type { IpcError } from "./types/generated/IpcError";
import type { IslandState } from "./types/generated/IslandState";
import type { LiveItem } from "./types/generated/LiveItem";
import type { ModuleInfo } from "./types/generated/ModuleInfo";
import type { MonitorInfo } from "./types/generated/MonitorInfo";
import type { PipMood } from "./types/generated/PipMood";
import type { Preferences } from "./types/generated/Preferences";
import type { SkillInstallResult } from "./types/generated/SkillInstallResult";
import type { SkillsState } from "./types/generated/SkillsState";

export type IpcResult<T> = { ok: true; value: T } | { ok: false; error: IpcError };

function toIpcError(raw: unknown): IpcError {
  if (
    typeof raw === "object" &&
    raw !== null &&
    typeof (raw as IpcError).code === "string" &&
    typeof (raw as IpcError).message === "string"
  ) {
    return raw as IpcError;
  }
  return { code: "unknown", message: typeof raw === "string" ? raw : "Unexpected error" };
}

/** The only place in the frontend allowed to call `invoke` (Rules.md §3). */
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<IpcResult<T>> {
  try {
    return { ok: true, value: await invoke<T>(command, args) };
  } catch (raw) {
    return { ok: false, error: toIpcError(raw) };
  }
}

/** Backend → frontend event names (see runtime.rs, island/controller.rs, actions.rs). */
export const events = {
  islandState: "island:state",
  islandDesktop: "island:desktop",
  liveItems: "live:items",
  pipMood: "pip:mood",
  preferences: "preferences:changed",
  skills: "skills:changed",
} as const;

export const ipc = {
  appInfo: () => call<AppInfo>("app_info"),
  islandGetState: () => call<IslandState>("island_get_state"),
  islandOnDesktop: () => call<boolean>("island_on_desktop"),
  islandSetPanelHeight: (height: number) => call<void>("island_set_panel_height", { height }),
  islandSetState: (state: IslandState) => call<void>("island_set_state", { state }),
  liveItems: () => call<LiveItem[]>("live_get_items"),
  pipMood: () => call<PipMood>("pip_get_mood"),
  preferencesGet: () => call<Preferences>("preferences_get"),
  preferencesSet: (preferences: Preferences) => call<void>("preferences_set", { preferences }),
  monitorsList: () => call<MonitorInfo[]>("monitors_list"),
  modulesList: () => call<ModuleInfo[]>("modules_list"),
  openSettings: () => call<void>("settings_open"),

  // Productivity module (Phase 2)
  timerGet: () => call<TimerSnapshot>("timer_get"),
  /** With a task, its title is the label and the session counts for it. */
  timerStart: (label: string, minutes: number, taskId: string | null = null) =>
    call<TimerSnapshot>("timer_start", { label, minutes, taskId }),
  timerDismissFinished: () => call<TimerSnapshot>("timer_dismiss_finished"),
  /** Picks or unpicks a task as one of today's (up to three) priorities. */
  taskSetPriority: (id: string, on: boolean) => call<Task[]>("task_set_priority", { id, on }),
  todaySummary: () => call<TodaySummary>("today_summary"),
  /** Sets a task's reminder time (unix ms), or clears it with null. */
  taskSetDue: (id: string, dueAt: number | null) => call<Task[]>("task_set_due", { id, dueAt }),

  // Quiet mode: pause routine prompts for a while, or hold them during focus.
  quietGet: () => call<QuietState>("quiet_get"),
  /** `until`: unix ms, or null to resume now. */
  quietPause: (until: number | null) => call<QuietState>("quiet_pause", { until }),
  quietHoldInFocus: (hold: boolean) => call<QuietState>("quiet_hold_in_focus", { hold }),
  quietHoldFullscreenApps: (hold: boolean) =>
    call<QuietState>("quiet_hold_fullscreen_apps", { hold }),

  // Your data: capture pause, keep-for limits, export, delete.
  privacyGet: () => call<PrivacySettings>("privacy_get"),
  privacySet: (settings: PrivacySettings) => call<PrivacySettings>("privacy_set", { settings }),
  /** Asks where to save; resolves to the path, or null if cancelled. */
  privacyExport: () => call<string | null>("privacy_export"),
  privacyDeleteAll: () => call<void>("privacy_delete_all", { confirmed: true }),

  // The optional calendar: nothing is fetched until the user adds one.
  calendarGet: () => call<CalendarState>("calendar_get"),
  calendarAddLink: (name: string, link: string) =>
    call<CalendarState>("calendar_add_link", { name, link }),
  /** Opens the file picker for a local .ics file. */
  calendarAddFile: (name: string) => call<CalendarState>("calendar_add_file", { name }),
  calendarRemove: (id: string) => call<CalendarState>("calendar_remove", { id }),
  calendarSetEnabled: (id: string, enabled: boolean) =>
    call<CalendarState>("calendar_set_enabled", { id, enabled }),
  calendarSetOptions: (remindMin: number, hideTitles: boolean) =>
    call<CalendarState>("calendar_set_options", { remindMin, hideTitles }),
  calendarRefresh: () => call<CalendarState>("calendar_refresh"),
  timerPause: () => call<TimerSnapshot>("timer_pause"),
  timerResume: () => call<TimerSnapshot>("timer_resume"),
  timerStop: () => call<TimerSnapshot>("timer_stop"),
  tasksList: () => call<Task[]>("tasks_list"),
  taskAdd: (title: string) => call<Task[]>("task_add", { title }),
  taskRename: (id: string, title: string) => call<Task[]>("task_rename", { id, title }),
  taskSetDone: (id: string, done: boolean) => call<Task[]>("task_set_done", { id, done }),
  taskDelete: (id: string) => call<Task[]>("task_delete", { id }),
  tasksReorder: (ids: string[]) => call<Task[]>("tasks_reorder", { ids }),
  clipboardList: () => call<ClipItem[]>("clipboard_list"),
  clipboardCopy: (id: string) => call<void>("clipboard_copy", { id }),
  clipboardSetPinned: (id: string, pinned: boolean) =>
    call<ClipItem[]>("clipboard_set_pinned", { id, pinned }),
  clipboardDelete: (id: string) => call<ClipItem[]>("clipboard_delete", { id }),
  clipboardClear: () => call<ClipItem[]>("clipboard_clear"),
  clipboardImage: (id: string) => call<string>("clipboard_image", { id }),
  mediaGet: () => call<NowPlaying | null>("media_get"),
  mediaCommand: (action: MediaAction) => call<NowPlaying | null>("media_command", { action }),

  // System & Dev Monitor (Phase 3)
  monitorGet: () => call<MonitorSnapshot>("monitor_get"),
  monitorSetView: (view: MonitorView) => call<void>("monitor_set_view", { view }),
  processKill: (pid: number) => call<ProcessInfo[]>("process_kill", { pid }),
  alertRuleSave: (rule: AlertRule) => call<AlertRule[]>("alert_rule_save", { rule }),
  alertRuleDelete: (id: string) => call<AlertRule[]>("alert_rule_delete", { id }),
  /** Remind again every `minutes` while an alert lasts (0 = say it once). */
  alertRepeatSet: (minutes: number) => call<number>("alert_repeat_set", { minutes }),
  dockerAction: (id: string, action: ContainerAction) =>
    call<void>("docker_action", { id, action }),
  githubTokenSet: (token: string | null) => call<void>("github_token_set", { token }),
  targetAdd: (kind: TargetKind, target: string) =>
    call<WatchTarget[]>("target_add", { kind, target }),
  targetDelete: (id: string) => call<WatchTarget[]>("target_delete", { id }),

  // Weather (opt-in, Open-Meteo)
  weatherGet: () => call<WeatherState | null>("weather_get"),
  weatherSearch: (query: string) => call<WeatherPlace[]>("weather_search", { query }),
  weatherRefresh: () => call<void>("weather_refresh"),

  // Skills (data-only add-ons in the Preku Skill Format)
  skillsGet: () => call<SkillsState>("skills_get"),
  /** Opens a file picker; `installed` is empty when the picker was cancelled. */
  skillInstall: () => call<SkillInstallResult>("skill_install"),
  skillSetEnabled: (id: string, enabled: boolean) =>
    call<SkillsState>("skill_set_enabled", { id, enabled }),
  skillRemove: (id: string) => call<SkillsState>("skill_remove", { id }),
  skillRemoveBroken: (file: string) => call<SkillsState>("skill_remove_broken", { file }),

  // Wellbeing (Phase 4)
  wellbeingGet: () => call<WellbeingState>("wellbeing_get"),
  waterAdd: (ml?: number) => call<WellbeingState>("water_add", { ml: ml ?? null }),
  waterUndo: () => call<WellbeingState>("water_undo"),
  /** `days`: weekday mask, bit 0 = Monday … bit 6 = Sunday (127 = every day). */
  habitAdd: (name: string, target: number, days = 127) =>
    call<WellbeingState>("habit_add", { name, target, days }),
  habitUpdate: (id: string, name: string, target: number, days: number) =>
    call<WellbeingState>("habit_update", { id, name, target, days }),
  habitDelete: (id: string) => call<WellbeingState>("habit_delete", { id }),
  habitLog: (id: string, delta: number) => call<WellbeingState>("habit_log", { id, delta }),
  wellbeingSettingsSet: (settings: WellbeingSettings) =>
    call<WellbeingState>("wellbeing_settings_set", { settings }),
  wellbeingPromptLater: (id: string) => call<WellbeingState>("wellbeing_prompt_later", { id }),
  wellbeingBreakDone: (kind: ReminderKind, completed: boolean) =>
    call<WellbeingState>("wellbeing_break_done", { kind, completed }),
  moodLog: (mood: string) => call<WellbeingState>("mood_log", { mood }),
  bridgeGet: () => call<BridgeState>("bridge_get"),
  bridgePairStart: () => call<BridgeState>("bridge_pair_start"),
  bridgePairCancel: () => call<BridgeState>("bridge_pair_cancel"),
  bridgeUnpair: (id: string) => call<BridgeState>("bridge_unpair", { id }),
  bridgeSendFiles: (paths: string[]) => call<BridgeState>("bridge_send_files", { paths }),
  bridgePickAndSend: () => call<BridgeState>("bridge_pick_and_send"),
  bridgeCancelTransfer: (id: string) => call<BridgeState>("bridge_cancel_transfer", { id }),
  /** Shows a received file in Explorer, or the inbox folder when `id` is null. */
  bridgeReveal: (id: string | null) => call<void>("bridge_reveal", { id }),
  bridgeSendText: (text: string) => call<BridgeState>("bridge_send_text", { text }),
  bridgeClearTexts: () => call<BridgeState>("bridge_clear_texts"),
  /** Opens a shared text in the browser; the backend only does it for a plain web link. */
  bridgeOpenLink: (id: string) => call<void>("bridge_open_link", { id }),
  settingsGet: <T>(key: string) => call<T | null>("settings_get", { key }),
  settingsSet: (key: string, value: unknown) => call<void>("settings_set", { key, value }),
};
