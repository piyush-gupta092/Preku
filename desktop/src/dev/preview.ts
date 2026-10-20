// Dev-only preview harness: mocks the Tauri backend so any window can be rendered in a plain
// browser (or headless Edge for screenshots). Open via the dev server:
//
//   http://localhost:1420/preview.html?window=island&state=expanded&items=2
//
// Query params: window (island|settings), state (hidden|compact|expanded),
// items (0-3 sample live items), mood (PipMood), position (left|center|right),
// tab (home|focus|tasks|clipboard), timer (running), media (none),
// open (module:tab, e.g. monitor:dev) to open a module view directly, dnd (1),
// tiles (comma list), radius (px), hide (comma list of activity keys).

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { LiveItem } from "../lib/types/generated/LiveItem";
import type { Preferences } from "../lib/types/generated/Preferences";
import kindWordsSkill from "../../../skills/examples/kind-words.json";
import wristSkill from "../../../skills/examples/wrist-and-eye-care.json";

const params = new URLSearchParams(location.search);
const now = Date.now();

const sampleItems: LiveItem[] = [
  {
    id: "wellbeing.reminder.eyes",
    module: "wellbeing",
    priority: "alert",
    title: "👀",
    subtitle: null,
    progress: null,
    expiresAt: null,
    createdAt: now + 1,
    kind: "reminder.eyes",
  },
  {
    id: "alert",
    module: "monitor",
    priority: "alert",
    title: "Disk almost full",
    subtitle: "C: has 3.2 GB left",
    progress: null,
    expiresAt: null,
    createdAt: now,
  },
  {
    id: "timer",
    module: "productivity",
    priority: "timer",
    title: "Focus",
    subtitle: "15 min left",
    progress: 0.4,
    expiresAt: null,
    createdAt: now - 1000,
  },
  {
    id: "info",
    module: "wellbeing",
    priority: "info",
    title: "Time for a sip of water with a deliberately long title",
    subtitle: "Demo item",
    progress: null,
    expiresAt: null,
    createdAt: now - 2000,
  },
];

const preferences: Preferences = {
  islandPosition: (params.get("position") as Preferences["islandPosition"]) ?? "center",
  monitor: null,
  theme: (params.get("theme") as Preferences["theme"]) ?? "system",
  launchAtLogin: false,
  modules: {},
  paused: false,
  sounds: true,
  notchModules: (params.get("tiles") ?? "productivity,monitor,wellbeing,bridge").split(","),
  pillRadius: Number(params.get("radius") ?? 18),
  notchActivities: Object.fromEntries(
    (params.get("hide") ?? "")
      .split(",")
      .filter(Boolean)
      .map((key) => [key, false]),
  ),
  weatherPlace:
    params.get("weather") === "off"
      ? null
      : { name: "Mumbai", detail: "Maharashtra, India", latitude: 19.07, longitude: 72.88 },
  temperatureUnit: "celsius",
  mascotStyle: (params.get("pip") as Preferences["mascotStyle"]) ?? "classic",
  mascotColor: params.get("pipColor") ?? "#ffb38a",
  // ?welcome=1 shows the first-run welcome in Settings.
  welcomed: params.get("welcome") !== "1",
  devTools: params.get("dev") !== "off",
  // ?thought=1 shows the thought for today at start.
  // ?moment=1: today's "A moment for you" is still due (the card shows when expanded).
  dailyThought: params.get("moment") === "1",
  // ?today=1 puts the Today card back on the home view.
  todayOnHome: params.get("today") === "1",
  savedThoughts: ["Rest is part of the work, not a pause from it."],
};

// ?timer=running|finished (finished: the "session done" card for the first task).
const timerRunning = params.get("timer") === "running";
const timerFinished = params.get("timer") === "finished";
const tasks = [
  {
    id: "t1",
    title: "Write the Phase 2 notes",
    done: false,
    createdAt: now,
    updatedAt: now,
    focusSessions: 2,
    focusS: 3000,
    priorityToday: true,
    dueAt: null,
  },
  {
    id: "t2",
    title: "Review clipboard privacy rules",
    done: false,
    createdAt: now,
    updatedAt: now,
    focusSessions: 0,
    focusS: 720,
    priorityToday: false,
    dueAt: null,
  },
  {
    id: "t3",
    title: "Reply to Sam about the demo",
    done: false,
    createdAt: now,
    updatedAt: now,
    focusSessions: 0,
    focusS: 0,
    priorityToday: false,
    dueAt: null,
  },
  {
    id: "t4",
    title: "Buy coffee beans",
    done: true,
    createdAt: now,
    updatedAt: now,
    focusSessions: 0,
    focusS: 0,
    priorityToday: false,
    dueAt: null,
  },
];
const clip = (id: string, preview: string, minutesAgo: number, pinned = false) => ({
  id,
  kind: "text",
  preview,
  width: null,
  height: null,
  pinned,
  createdAt: now - minutesAgo * 60_000,
});
const clipboard = [
  clip("c1", 'git commit -m "feat(productivity): focus timer"', 2, true),
  clip("c2", "https://github.com/preku/preku/issues/42", 5),
  { ...clip("c3", "", 14), kind: "image", width: 1920, height: 1080 },
  clip("c4", "The quick brown fox jumps over the lazy dog, again and again and again.", 60),
];

const GB = 1024 ** 3;
const history = Array.from({ length: 60 }, (_, i) => ({
  ts: now - (59 - i) * 2000,
  cpu: 18 + 12 * Math.sin(i / 5) + (i % 7) * 2,
  memUsed: 9.4 * GB + Math.sin(i / 9) * 0.3 * GB,
  memTotal: 16 * GB,
  diskName: "C:\\",
  diskUsed: 431 * GB,
  diskTotal: 476 * GB,
  netRx: 120_000 + 90_000 * Math.max(0, Math.sin(i / 3)),
  netTx: 30_000 + 20_000 * Math.max(0, Math.cos(i / 4)),
  battery: { percent: 82, charging: false, secsLeft: 3 * 3600 + 20 * 60 },
}));
const monitorSnapshot = {
  history,
  historyHour: history.slice(0, 40),
  alertRepeatMin: 0,
  processes: [
    { pid: 4120, name: "chrome.exe", cpu: 6.4, memory: 1.2 * GB },
    { pid: 9880, name: "Code.exe", cpu: 4.1, memory: 0.9 * GB },
    { pid: 2216, name: "Spotify.exe", cpu: 1.2, memory: 0.31 * GB },
    { pid: 7008, name: "msedgewebview2.exe", cpu: 0.8, memory: 0.18 * GB },
    { pid: 1200, name: "explorer.exe", cpu: 0.3, memory: 0.12 * GB },
  ],
  rules: [
    { id: "default-cpu", metric: "cpu", op: "above", threshold: 90, durationS: 30, enabled: true },
    {
      id: "default-memory",
      metric: "memory",
      op: "above",
      threshold: 90,
      durationS: 60,
      enabled: true,
    },
    { id: "default-disk", metric: "disk", op: "above", threshold: 95, durationS: 0, enabled: true },
    {
      id: "default-battery",
      metric: "battery",
      op: "below",
      threshold: 15,
      durationS: 0,
      enabled: false,
    },
    {
      id: "net",
      metric: "netDown",
      op: "above",
      threshold: 50,
      durationS: 10,
      enabled: true,
    },
    {
      id: "free",
      metric: "diskFree",
      op: "below",
      threshold: 20,
      durationS: 0,
      enabled: true,
    },
  ],
  docker: {
    available: true,
    containers: [
      { id: "c1", name: "postgres", image: "postgres:16", state: "running", status: "Up 3 hours" },
      { id: "c2", name: "redis", image: "redis:7", state: "running", status: "Up 3 hours" },
      {
        id: "c3",
        name: "worker",
        image: "app/worker",
        state: "exited",
        status: "Exited (1) 4 minutes ago",
      },
    ],
  },
  ci: [
    {
      repo: "preku/preku",
      run: { id: 1, name: "CI", branch: "main", status: "in_progress", conclusion: null, url: "" },
      error: null,
    },
    {
      repo: "tauri-apps/tauri",
      run: {
        id: 2,
        name: "test",
        branch: "dev",
        status: "completed",
        conclusion: "success",
        url: "",
      },
      error: null,
    },
  ],
  uptime: [
    {
      url: "https://example.com",
      up: true,
      latencyMs: 182,
      httpStatus: 200,
      error: null,
      checkedAt: now,
    },
    {
      url: "http://localhost:3000/health",
      up: false,
      latencyMs: null,
      httpStatus: null,
      error: "Can't connect",
      checkedAt: now,
    },
  ],
  watches: [
    {
      id: "w1",
      command: "cargo build --release",
      state: "succeeded",
      code: 0,
      startedAt: now,
      durationMs: 83_000,
    },
    { id: "w2", command: "npm test", state: "failed", code: 1, startedAt: now, durationMs: 12_000 },
  ],
  targets: [
    { id: "t1", kind: "github", target: "preku/preku", intervalS: 60 },
    { id: "t2", kind: "github", target: "tauri-apps/tauri", intervalS: 60 },
    { id: "t3", kind: "uptime", target: "https://example.com", intervalS: 60 },
    { id: "t4", kind: "uptime", target: "http://localhost:3000/health", intervalS: 60 },
  ],
  githubTokenSet: false,
};

const wellbeingState = {
  settings: {
    eyesEnabled: true,
    eyesEveryMin: 20,
    standEnabled: true,
    standEveryMin: 50,
    waterEnabled: true,
    waterEveryMin: 60,
    waterGoalMl: 2000,
    glassMl: 250,
    seatedMovement: params.get("seated") === "1",
    postureEnabled: true,
    postureEveryMin: 45,
    screenBalanceEnabled: true,
    screenBalanceMin: 120,
    bedtimeEnabled: true,
    bedtime: "23:00",
    windDownMin: 30,
    mealsEnabled: true,
    mealTimes: ["13:00", "20:00"],
    moodCheckInEnabled: true,
    moodCheckInTime: "18:00",
    quietHoursEnabled: false,
    quietStart: "22:30",
    quietEnd: "07:00",
    snoozeMin: 10,
    encouragementEnabled: true,
    screenTimeEnabled: true,
  },
  waterMl: 1250,
  habits: [
    {
      id: "h1",
      name: "Stretch",
      target: 1,
      countToday: 1,
      doneToday: true,
      streak: 6,
      days: 127,
      scheduledToday: true,
      cameBack: false,
    },
    {
      id: "h2",
      name: "Read 20 pages",
      target: 1,
      countToday: 0,
      doneToday: false,
      streak: 12,
      days: 127,
      scheduledToday: true,
      cameBack: false,
    },
    {
      id: "h3",
      name: "Push-ups",
      target: 3,
      countToday: 2,
      doneToday: false,
      streak: 0,
      days: 0b0010101,
      scheduledToday: true,
      cameBack: false,
    },
  ],
  screenApps: [
    { app: "Code", seconds: 2 * 3600 + 14 * 60 },
    { app: "Chrome", seconds: 1 * 3600 + 3 * 60 },
    { app: "Spotify", seconds: 41 * 60 },
    { app: "Slack", seconds: 22 * 60 },
    { app: "Explorer", seconds: 6 * 60 },
  ],
  screenTotalS: 4 * 3600 + 26 * 60,
  screenWeekS: [5.1, 6.3, 4.2, 7.0, 3.1, 1.2, 4.4].map((h) => Math.round(h * 3600)),
  screenSupported: true,
  dnd: params.get("dnd") === "1",
  nextBreakS: 7 * 60,
  // ?prompt=eyes|stand|water|bedtime|moodCheckIn|… shows a companion invitation.
  prompt: params.get("prompt")
    ? { id: "p1", kind: params.get("prompt"), activeMin: 52, sinceMin: 62, createdAt: now }
    : null,
  breaksToday: 3,
  moodToday: "good",
  week: [1250, 2000, 1750, 0, 2250, 1500, 1250].map((waterMl, i) => {
    const day = new Date(now - (6 - i) * 86_400_000);
    return {
      day: `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, "0")}-${String(day.getDate()).padStart(2, "0")}`,
      waterMl,
      breaks: [2, 4, 3, 0, 5, 1, 3][i],
      habitsDone: [1, 2, 2, 0, 3, 1, 1][i],
      mood: ["good", "great", null, null, "okay", "good", "good"][i],
    };
  }),
};

// ?phone=online|offline|pairing|none shows the phone bridge in that state.
const phoneParam = params.get("phone") ?? "online";
const qrMock =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 21 21"><rect width="21" height="21" fill="#fff"/>' +
  Array.from(
    { length: 120 },
    (_, i) =>
      `<rect x=\"${(i * 7) % 21}\" y=\"${(i * 3) % 21}\" width=\"1\" height=\"1\" fill=\"#14141a\"/>`,
  ).join("") +
  "</svg>";
// ?calendar=on shows the optional calendar with two sources; default: none added.
const calendarOn = params.get("calendar") === "on";
const todayAt = (h: number, m: number) => new Date(new Date(now).setHours(h, m, 0, 0)).getTime();
const calendarState = {
  settings: {
    sources: calendarOn
      ? [
          { id: "c1", name: "Work", kind: "link", location: "calendar.google.com", enabled: true },
          {
            id: "c2",
            name: "Family",
            kind: "file",
            location: "C:/Users/you/Documents/family.ics",
            enabled: true,
          },
        ]
      : [],
    remindMin: 10,
    hideTitles: false,
  },
  statuses: calendarOn
    ? [
        { id: "c1", lastOk: now - 4 * 60_000, error: null },
        { id: "c2", lastOk: now - 4 * 60_000, error: null },
      ]
    : [],
  today: calendarOn
    ? [
        {
          id: "e0",
          title: "Asha's birthday",
          start: todayAt(0, 0),
          end: todayAt(24, 0),
          allDay: true,
          sourceName: "Family",
        },
        {
          id: "e1",
          title: "Stand-up",
          start: todayAt(10, 0),
          end: todayAt(10, 15),
          allDay: false,
          sourceName: "Work",
        },
        {
          id: "e2",
          title: "Design review with the mobile team",
          start: now + 20 * 60_000,
          end: now + 80 * 60_000,
          allDay: false,
          sourceName: "Work",
        },
      ]
    : [],
  next: calendarOn
    ? {
        id: "e2",
        title: "Design review with the mobile team",
        start: now + 20 * 60_000,
        end: now + 80 * 60_000,
        allDay: false,
        sourceName: "Work",
      }
    : null,
};

const bridgeState = {
  pcName: "DESK-PC",
  phones:
    phoneParam === "none" || phoneParam === "pairing"
      ? []
      : [
          {
            id: "ph1",
            name: "Pixel 8",
            pairedAt: now - 86_400_000,
            lastSeen: now - 25 * 60_000,
            online: phoneParam === "online",
            kind: "app",
            status:
              phoneParam === "online" ? { battery: 82, charging: true, network: "wifi" } : null,
          },
        ],
  pairing:
    phoneParam === "pairing"
      ? {
          link: "preku://pair?host=192.168.1.20&port=47823",
          qrSvg: qrMock,
          address: "192.168.1.20:47823",
          webLink: "https://192.168.1.20:47824/#pair=abc",
          webQrSvg: qrMock.replace(/(i * 7)/g, "(i * 5)"),
          expiresAt: now + 95_000,
        }
      : null,
  listening: phoneParam !== "none",
  error: null,
  inbox: "C:\Users\you\Downloads\Preku",
  texts:
    phoneParam === "online"
      ? [
          {
            id: "x1",
            text: "https://example.com/tickets?id=42",
            isLink: true,
            direction: "receive",
            deviceName: "iPhone",
            at: now - 60_000,
          },
          {
            id: "x2",
            text: "Meet at 5, gate B. Bring the charger.",
            isLink: false,
            direction: "send",
            deviceName: "iPhone",
            at: now - 9 * 60_000,
          },
        ]
      : [],
  transfers:
    phoneParam === "online"
      ? [
          {
            id: "t1",
            name: "Project report.pdf",
            size: 4_800_000,
            bytes: 2_100_000,
            direction: "send",
            status: "active",
            deviceName: "Pixel 8",
            error: null,
            updatedAt: now,
          },
          {
            id: "t2",
            name: "IMG_20261009_181233.jpg",
            size: 3_145_728,
            bytes: 3_145_728,
            direction: "receive",
            status: "done",
            deviceName: "Pixel 8",
            error: null,
            updatedAt: now - 60_000,
          },
          {
            id: "t3",
            name: "voice-note.m4a",
            size: 900_000,
            bytes: 300_000,
            direction: "receive",
            status: "failed",
            deviceName: "Pixel 8",
            error: "disconnected",
            updatedAt: now - 120_000,
          },
        ]
      : [],
};

// ?skills=0 shows the Skills page empty; otherwise the two example skills are installed.
/** As the backend sends a skill: every contribution list present, even when empty. */
const asInstalled = (skill: object) => {
  const raw = skill as { contributes: object };
  return { ...raw, contributes: { thoughts: [], nudges: [], sounds: [], ...raw.contributes } };
};
const skillsState = {
  skills:
    params.get("skills") === "0"
      ? []
      : [
          { manifest: asInstalled(kindWordsSkill), enabled: true },
          { manifest: asInstalled(wristSkill), enabled: params.get("skills") !== "off" },
        ],
  broken:
    params.get("skills") === "broken"
      ? [{ file: "my-skill.json", problems: ["nudges[0].everyMin must be 5–240 minutes"] }]
      : [],
};

const responses: Record<string, unknown> = {
  bridge_get: bridgeState,
  skills_get: skillsState,
  calendar_get: calendarState,
  // ?quiet=paused shows reminders paused for another 25 minutes.
  quiet_get: {
    until: params.get("quiet") === "paused" ? now + 25 * 60_000 : null,
    holdInFocus: true,
    holdFullscreenApps: false,
    focusing: timerRunning,
    screenBusy: false,
  },
  privacy_get: { clipboardPaused: false, clipboardKeepDays: 0, usageKeepDays: 0 },
  today_summary: { focusS: 3000, sessions: 2, tasksDone: 2, priorities: 3, prioritiesDone: 1 },
  weather_get:
    params.get("weather") === "off"
      ? null
      : {
          place: "Mumbai",
          temperature: 29.9,
          high: 37.8,
          low: 28.0,
          code: 0,
          isDay: false,
          unit: "celsius",
          updatedAt: now,
        },
  weather_search: [
    { name: "Mumbai", detail: "Maharashtra, India", latitude: 19.07, longitude: 72.88 },
  ],
  weather_refresh: null,
  wellbeing_get: wellbeingState,
  wellbeing_prompt_later: { ...wellbeingState, prompt: null },
  wellbeing_break_done: { ...wellbeingState, prompt: null },
  mood_log: { ...wellbeingState, prompt: null },
  water_add: { ...wellbeingState, prompt: null },
  monitor_get: monitorSnapshot,
  monitor_set_view: null,
  app_info: {
    name: "Preku",
    version: "0.1.0",
    shortcut: "ctrl+alt+p",
    captureShortcut: "ctrl+alt+n",
  },
  island_get_state: params.get("state") ?? "expanded",
  island_set_state: null,
  island_on_desktop: params.get("desktop") !== "0",
  island_set_panel_height: null,
  live_get_items: sampleItems.slice(0, Number(params.get("items") ?? 1)),
  pip_get_mood: params.get("mood") ?? "idle",
  preferences_get: preferences,
  preferences_set: null,
  monitors_list: [{ name: "\\\\.\\DISPLAY1", primary: true, width: 1920, height: 1200 }],
  modules_list: [
    { id: "productivity", enabled: true, status: { state: "running" } },
    { id: "monitor", enabled: true, status: { state: "running" } },
    { id: "wellbeing", enabled: true, status: { state: "running" } },
    {
      id: "weather",
      enabled: params.get("weather") !== "off",
      status: { state: params.get("weather") === "off" ? "stopped" : "running" },
    },
    { id: "bridge", enabled: true, status: { state: "running" } },
    { id: "demo", enabled: true, status: { state: "running" } },
  ],
  settings_open: null,
  timer_get: timerRunning
    ? {
        status: "running",
        label: "Write the Phase 2 notes",
        totalS: 1500,
        remainingS: 912,
        endsAt: now + 912_000,
        taskId: "t1",
        finished: null,
      }
    : {
        status: "idle",
        label: "",
        totalS: 0,
        remainingS: 0,
        endsAt: null,
        taskId: null,
        finished: timerFinished
          ? { label: "Write the Phase 2 notes", taskId: "t1", totalS: 1500, focusedS: 1500 }
          : null,
      },
  tasks_list: tasks,
  clipboard_list: clipboard,
  clipboard_image:
    "data:image/svg+xml;base64," +
    btoa(
      '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="9"><rect width="16" height="9" fill="#7c5cff"/></svg>',
    ),
  media_get:
    params.get("media") === "none"
      ? null
      : {
          title: params.get("title") ?? "Midnight City",
          artist: "M83",
          app: "Spotify",
          playing: true,
          canNext: true,
          canPrevious: true,
        },
};

if (params.get("moment") === "1") localStorage.removeItem("preku.moment.day");
const tab = params.get("tab");
if (tab) localStorage.setItem("preku.island.tab", tab);

const settingsPage = params.get("page");
if (settingsPage) localStorage.setItem("preku.settings.page", settingsPage);
// ?pairTarget=web shows the iPhone QR tab while pairing.
const pairTarget = params.get("pairTarget");
if (pairTarget) localStorage.setItem("preku.bridge.pairTarget", pairTarget);
localStorage.setItem("preku.settings.module", params.get("module") ?? "");

const open = params.get("open");
if (open) sessionStorage.setItem("preku.preview.open", open);
else sessionStorage.removeItem("preku.preview.open");

mockWindows(params.get("window") ?? "island");
mockIPC((cmd) => (cmd in responses ? responses[cmd] : null), { shouldMockEvents: true });

if (params.get("window") === "gallery") {
  await import("../styles/tokens.css");
  await import("../styles/base.css");
  const { mount } = await import("svelte");
  const { default: PipGallery } = await import("./PipGallery.svelte");
  mount(PipGallery, { target: document.getElementById("app")! });
} else {
  await import("../main");
  // ?session=reset|eyes|stand…&phase=break|done|ask shows a companion break in progress.
  const session = params.get("session");
  if (session) {
    const { companion } = await import("../lib/stores/companion.svelte");
    const phase = (params.get("phase") ?? "break") as "ask" | "break" | "done";
    companion.session = {
      kind: session as never,
      phase,
      endsAt: phase === "break" ? Date.now() + 47_000 : null,
    };
  }
}
