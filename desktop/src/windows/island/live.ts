import type { LiveItem } from "../../lib/types/generated/LiveItem";

const MODULE_ACCENTS: Record<string, string> = {
  productivity: "var(--mod-productivity)",
  bridge: "var(--mod-bridge)",
  monitor: "var(--mod-monitor)",
  wellbeing: "var(--mod-wellbeing)",
  weather: "var(--mod-weather)",
  skills: "var(--brand)",
};

/** Each module has its own accent (Design.md §2.3); anything else uses the brand color. */
export function accentFor(module: string): string {
  return MODULE_ACCENTS[module] ?? "var(--brand)";
}

/** Urgent items pop the island out even when nobody is hovering it. */
export function isUrgent(item: LiveItem): boolean {
  return item.priority === "alert" || item.priority === "incomingCall";
}

/** Text for an item: translated for known `kind`s, otherwise what the backend sent. */
export function liveText(
  item: LiveItem,
  messages: Record<string, { title: string; subtitle?: string }>,
): { title: string; subtitle: string | null } {
  const message = item.kind ? messages[item.kind] : undefined;
  return message
    ? { title: message.title, subtitle: message.subtitle ?? null }
    : { title: item.title, subtitle: item.subtitle };
}

export function nextRotation(index: number, count: number): number {
  return count <= 1 ? 0 : (index + 1) % count;
}

/** Kinds of things that can appear in the notch, which the user can switch off in Settings. */
export const ACTIVITIES = [
  { key: "focusTimer", module: "productivity", ids: ["productivity.timer"] },
  { key: "focusDone", module: "productivity", ids: ["productivity.timer-done"] },
  { key: "nextTask", module: "productivity", ids: ["productivity.task"] },
  { key: "taskReminders", module: "productivity", ids: ["productivity.due"] },
  { key: "nowPlaying", module: "productivity", ids: ["productivity.media"] },
  { key: "calendarEvents", module: "productivity", ids: ["productivity.calendar"] },
  { key: "systemAlerts", module: "monitor", ids: ["monitor.alert."] },
  { key: "docker", module: "monitor", ids: ["monitor.docker."] },
  { key: "ci", module: "monitor", ids: ["monitor.ci."] },
  { key: "uptime", module: "monitor", ids: ["monitor.uptime."] },
  { key: "commandWatch", module: "monitor", ids: ["monitor.watch."] },
  {
    key: "breakReminders",
    module: "wellbeing",
    ids: [
      "wellbeing.reminder.eyes",
      "wellbeing.reminder.stand",
      "wellbeing.reminder.posture",
      "wellbeing.reminder.screen",
    ],
  },
  { key: "waterReminders", module: "wellbeing", ids: ["wellbeing.reminder.water"] },
  {
    key: "routineReminders",
    module: "wellbeing",
    ids: ["wellbeing.reminder.bedtime", "wellbeing.reminder.meal", "wellbeing.reminder.mood"],
  },
  { key: "weather", module: "weather", ids: ["weather."] },
  { key: "skillNudges", module: "skills", ids: ["skill."] },
  { key: "fileTransfers", module: "bridge", ids: ["bridge.transfer."] },
  { key: "demo", module: "demo", ids: ["demo."] },
] as const;

export type ActivityKey = (typeof ACTIVITIES)[number]["key"];

/** The activity a live item belongs to (ids ending in "." match as prefixes). */
export function activityOf(item: Pick<LiveItem, "id">): ActivityKey | null {
  const found = ACTIVITIES.find((activity) =>
    activity.ids.some((id) => (id.endsWith(".") ? item.id.startsWith(id) : item.id === id)),
  );
  return found?.key ?? null;
}

/** Whether an item may appear in the notch. Unknown items and unset keys are shown. */
export function isShown(
  item: Pick<LiveItem, "id">,
  settings: { [key in string]?: boolean },
): boolean {
  const key = activityOf(item);
  return key === null || settings[key] !== false;
}
