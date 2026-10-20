<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { events, ipc } from "../../lib/ipc";
  import { greetingFor, t } from "../../lib/i18n";
  import type { IslandPosition } from "../../lib/types/generated/IslandPosition";
  import type { IslandState } from "../../lib/types/generated/IslandState";
  import type { LiveItem } from "../../lib/types/generated/LiveItem";
  import type { PipMood } from "../../lib/types/generated/PipMood";
  import type { PipMood as AnyMood } from "../../pip/renderer";
  import type { Preferences } from "../../lib/types/generated/Preferences";
  import Pip from "../../pip/Pip.svelte";
  import Icon from "../../lib/Icon.svelte";
  import Marquee from "../../lib/Marquee.svelte";
  import { playComplete, playNotes, playPopup } from "../../lib/sound";
  import { nudgeSound, thoughtOn, thoughtPool } from "../../lib/skills";
  import { skills } from "../../lib/stores/skills.svelte";
  import { dayKey, momentDue, saveThought, skyLine } from "../../lib/moment";
  import MomentCard from "./MomentCard.svelte";
  import { productivity } from "../../lib/stores/productivity.svelte";
  import { monitor } from "../../lib/stores/monitor.svelte";
  import { wellbeing } from "../../lib/stores/wellbeing.svelte";
  import { calendar } from "../../lib/stores/calendar.svelte";
  import { quiet } from "../../lib/stores/quiet.svelte";
  import QuietButton from "./QuietButton.svelte";
  import TodayCard from "./TodayCard.svelte";
  import { weather } from "../../lib/stores/weather.svelte";
  import { bridge } from "../../lib/stores/bridge.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { mascot } from "../../lib/stores/mascot.svelte";
  import { condition, formatTemp, weatherIcon } from "../../lib/weather";
  import HabitsTab from "./wellbeing/HabitsTab.svelte";
  import TodayTab from "./wellbeing/TodayTab.svelte";
  import { formatPercent, percentOf } from "../../lib/format";
  import type { MonitorView } from "../../lib/types/generated/MonitorView";
  import AlertsTab from "./monitor/AlertsTab.svelte";
  import DevTab from "./monitor/DevTab.svelte";
  import OverviewTab from "./monitor/OverviewTab.svelte";
  import ProcessesTab from "./monitor/ProcessesTab.svelte";
  import ClipboardTab from "./tabs/ClipboardTab.svelte";
  import FocusTab from "./tabs/FocusTab.svelte";
  import NowPlayingCard from "./tabs/NowPlayingCard.svelte";
  import DanceStage from "./DanceStage.svelte";
  import ModuleMascot from "../../lib/ModuleMascot.svelte";
  import CompanionCard from "./wellbeing/CompanionCard.svelte";
  import { companion } from "../../lib/stores/companion.svelte";
  import { displayMoodFor, isNudge, nudgeLine } from "../../lib/companion";
  import MessagesTab from "./tabs/MessagesTab.svelte";
  import TasksTab from "./tabs/TasksTab.svelte";
  import { formatClock } from "./tabs/helpers";
  import { accentFor, isShown, isUrgent, liveText, nextRotation } from "./live";
  import {
    WHEEL_THRESHOLD,
    canScrollWithin,
    classifyHandleRelease,
    dragOffset,
    isPullDown,
    type ScrollBox,
  } from "./gestures";

  /** Collapse to hidden this long after the cursor leaves the compact pill (Architecture.md §3.1). */
  const LEAVE_DELAY_MS = 1500;
  /** The compact pill cycles through live items this often (Architecture.md §3.3). */
  const ROTATE_MS = 5000;
  /** An urgent item pops the island out for this long when the user isn't hovering. */
  const POP_MS = 5000;
  /** The pill opened from the top edge closes after this long if the pointer never enters it. */
  const EDGE_IDLE_MS = 4000;

  /** Home view tiles, one per module (Design.md §5). Only live modules open a view. */
  const modules = [
    { id: "productivity", phase: 2 },
    { id: "monitor", phase: 3 },
    { id: "wellbeing", phase: 4 },
    { id: "bridge", phase: 6 },
  ] as const;

  /** Each live module's views, opened from its tile (first = default). */
  const moduleTabs = {
    productivity: ["focus", "tasks", "clipboard", "messages"],
    monitor: ["overview", "processes", "dev", "alerts"],
    wellbeing: ["today", "habits"],
  } as const;
  type OpenModule = keyof typeof moduleTabs;
  const tabIcons: Record<string, string> = {
    focus: "focus",
    tasks: "tasks",
    clipboard: "clipboard",
    messages: "bridge",
    overview: "gauge",
    processes: "list",
    dev: "code",
    alerts: "bell",
    today: "home",
    habits: "check",
    screen: "monitor",
    reminders: "bell",
  };
  const tabLabels: Record<string, string> = {
    ...t.tabs,
    ...t.monitor.tabs,
    ...t.wellbeing.tabs,
  };

  let island = $state<IslandState>("hidden");
  /** The thin hidden line is only drawn while the Windows desktop is in front. */
  let onDesktop = $state(true);
  let items = $state<LiveItem[]>([]);
  let rotation = $state(0);
  let mood = $state<PipMood>("idle");
  let position = $state<IslandPosition>("center");
  let shortcut = $state<string | null>(null);
  let now = $state(new Date());
  let hovering = false;
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;
  let bigPip: Pip | undefined = $state();
  // Tall bodies (arms and legs) get a bigger canvas so the face stays readable; the header row
  // keeps its height (see .pet).
  const headerPip = $derived(mascot.style === "classic" ? 40 : 56);
  /** `null` shows the tiles; otherwise that module's view, on `tab`. */
  let openModule = $state<OpenModule | null>(null);
  let tab = $state<string>("");
  /** Each module reopens on its last tab, even after a restart (per-device convenience). */
  const TABS_KEY = "preku.island.tabs";
  function savedTabs(): Record<string, string> {
    try {
      const value = JSON.parse(localStorage.getItem(TABS_KEY) ?? "{}");
      return typeof value === "object" && value ? value : {};
    } catch {
      return {};
    }
  }
  let lastTabs = savedTabs();

  function openModuleView(id: OpenModule, next?: string) {
    const tabs: readonly string[] = moduleTabs[id];
    const remembered = lastTabs[id];
    const chosen =
      next && tabs.includes(next)
        ? next
        : remembered && tabs.includes(remembered)
          ? remembered
          : tabs[0]!;
    openModule = id;
    tab = chosen;
    lastTabs = { ...lastTabs, [id]: chosen };
    try {
      localStorage.setItem(TABS_KEY, JSON.stringify(lastTabs));
    } catch {
      // Storage unavailable: the tab just isn't remembered.
    }
  }

  // Dev-only: the preview harness (src/dev/preview.ts) can open a module view directly.
  if (import.meta.env.DEV) {
    const [id, next] = (sessionStorage.getItem("preku.preview.open") ?? "").split(":");
    if (id && id in moduleTabs) openModuleView(id as OpenModule, next);
  }

  /** What the Productivity tile says: the running timer, else the next task. */
  const productivityStatus = $derived.by(() => {
    const timer = productivity.timer;
    if (timer.status !== "idle") {
      const state = timer.status === "paused" ? t.home.paused : t.home.focusing;
      return `${state} · ${formatClock(timer.remainingS)}`;
    }
    const next = calendar.state?.next;
    if (next && next.start - now.getTime() < 60 * 60_000 && next.end > now.getTime()) {
      const time = new Date(next.start).toLocaleTimeString([], {
        hour: "numeric",
        minute: "2-digit",
      });
      return calendar.state?.settings.hideTitles
        ? t.calendar.tileNextPrivate(time)
        : t.calendar.tileNext(next.title, time);
    }
    return productivity.tasks.find((task) => !task.done)?.title ?? t.home.productivityIdle;
  });

  /**
   * Pip dances while music plays, as long as nothing more important is going on (alerts,
   * sad news and focus sessions keep their own mood).
   */

  // Which kinds of notch activity the user wants (Settings → Modules).
  let notchActivities = $state<{ [key in string]?: boolean }>({});
  /** The wellbeing companion has something to say (a prompt, a break, a kind word). */
  const companionShown = $derived(
    companion.session !== null ||
      (companion.prompt !== null &&
        isShown({ id: `wellbeing.reminder.${promptKey(companion.prompt.kind)}` }, notchActivities)),
  );

  const displayMood = $derived<AnyMood>(
    displayMoodFor({
      mood,
      playing: productivity.media?.playing ?? false,
      session: companion.session,
      prompt: companion.prompt,
      promptShown: companionShown,
      now: now.getTime(),
    }),
  );

  /** What the Wellbeing tile says: water progress and today's habits. */
  const wellbeingStatus = $derived.by(() => {
    const state = wellbeing.state;
    if (!state) return wellbeing.error ?? t.wellbeing.tileIdle;
    const done = state.habits.filter((h) => h.doneToday).length;
    return t.wellbeing.tile(state.waterMl, state.settings.waterGoalMl, done, state.habits.length);
  });

  /** What the Phone tile says: battery and network, or how to get there. */
  const phoneStatus = $derived.by(() => {
    const phone = bridge.phone;
    if (!phone) return t.bridge.tileNone;
    if (!phone.online || !phone.status) return t.bridge.tileOffline;
    return t.bridge.tileStatus(phone.status.battery, t.bridge.networks[phone.status.network] ?? "");
  });

  /** Files dragged over the island: offer to send them to the connected phone. */
  let droppingFiles = $state(false);
  const dropPhone = $derived(bridge.state?.phones.find((p) => p.online)?.name ?? null);
  onMount(() => {
    let off: (() => void) | undefined;
    void (async () => {
      try {
        off = await getCurrentWebview().onDragDropEvent((event) => {
          const kind = event.payload.type;
          if (kind === "enter" || kind === "over") droppingFiles = true;
          else if (kind === "leave") droppingFiles = false;
          else if (kind === "drop") {
            droppingFiles = false;
            if (dropPhone && event.payload.paths.length > 0) {
              void bridge.run(ipc.bridgeSendFiles(event.payload.paths));
            }
          }
        });
      } catch {
        // No drag and drop (preview harness).
      }
    })();
    return () => off?.();
  });

  /** Opens Settings on a module's page (Settings follows these keys, even if already open). */
  function openModuleSettings(id: string) {
    try {
      localStorage.setItem("preku.settings.page", "modules");
      localStorage.setItem("preku.settings.module", id);
    } catch {
      // Storage unavailable: Settings opens on its last page.
    }
    void ipc.openSettings();
  }

  /** What the Monitor tile says: live CPU and memory. */
  const monitorStatus = $derived.by(() => {
    const s = monitor.latest;
    if (!s) return monitor.error ?? t.monitor.tileIdle;
    return t.monitor.tile(formatPercent(s.cpu), formatPercent(percentOf(s.memUsed, s.memTotal)));
  });

  /** "screenBalance" → "screen" (the live-item id suffix). */
  function promptKey(kind: string): string {
    return kind === "screenBalance" ? "screen" : kind === "moodCheckIn" ? "mood" : kind;
  }

  /** The dance stage replaces the tiles while open (click the now-playing card). */
  let stage = $state(
    // Dev preview: ?stage=1 opens it (preview.html can't click).
    import.meta.env.DEV && new URLSearchParams(location.search).get("stage") === "1",
  );
  // Closes when the music goes away, the island collapses, or a module view opens. Only on
  // changes: music that hasn't loaded yet doesn't count as "went away".
  let hadMedia = false;
  let wasExpanded = false;
  $effect(() => {
    const media = productivity.media;
    if (media) hadMedia = true;
    else if (hadMedia) {
      hadMedia = false;
      stage = false;
    }
  });
  $effect(() => {
    if (island === "expanded") wasExpanded = true;
    else if (wasExpanded) {
      wasExpanded = false;
      stage = false;
    }
  });
  $effect(() => {
    if (openModule !== null) stage = false;
  });

  /** Panel heights (px): a short single row of tiles on the home view, more for modules. */
  const HOME_HEIGHT = 232;
  /** "A moment for you" is showing instead of the home view (see the moment block below). */
  let momentOpen = $state(false);
  const MOMENT_HEIGHT = 318;
  const NOW_PLAYING_HEIGHT = 60;
  /** The Today card (next action + recap) above the tiles, if the user wants it there. */
  const TODAY_HEIGHT = 68;
  let todayOnHome = $state(false);
  // Hidden while Productivity is off (its store then carries the reason).
  const todayShown = $derived(todayOnHome && productivity.error === null);
  /** The dance stage is taller than the row of tiles it replaces. */
  const STAGE_EXTRA = 44;
  const MODULE_HEIGHT = 360;
  const panelHeight = $derived(
    openModule !== null
      ? MODULE_HEIGHT
      : momentOpen
        ? MOMENT_HEIGHT
        : HOME_HEIGHT +
          (productivity.media ? NOW_PLAYING_HEIGHT : 0) +
          (todayShown && !stage && !companionShown ? TODAY_HEIGHT : 0) +
          (stage || companionShown ? STAGE_EXTRA : 0),
  );

  // A break started from elsewhere (the pill, the Today tab) shows on the home view.
  $effect(() => {
    if (companion.session) openModule = null;
  });
  // The window is sized by the backend, so it has to know the height before/while expanding.
  let sentHeight = 0;
  $effect(() => {
    if (panelHeight !== sentHeight) {
      sentHeight = panelHeight;
      void ipc.islandSetPanelHeight(panelHeight);
    }
  });

  // Tell the monitor how closely we're looking, so it samples fast only while visible
  // and reads the process table only on the Processes tab (Rules.md §5).
  let sentView: MonitorView | null = null;
  $effect(() => {
    const wanted: MonitorView =
      island === "expanded" && openModule === "monitor"
        ? tab === "processes"
          ? "processes"
          : "overview"
        : "off";
    if (wanted !== sentView) {
      sentView = wanted;
      void ipc.monitorSetView(wanted);
    }
  });

  let sounds = true;
  const seenUrgent = new Set<string>();

  // Bottom handle: drag/push the panel up (or click it) to tuck the island away.
  let dragStartY: number | null = null;
  let dragY = $state(0);
  let wheelSum = 0;
  let wheelReset: ReturnType<typeof setTimeout> | undefined;

  const time = $derived(now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }));
  // User customisation (Settings → Notch): which activities, tiles and corner radius.
  let notchModules = $state<string[]>(["productivity", "monitor", "wellbeing", "bridge"]);
  let pillRadius = $state(18);
  /** Bumped by the quick-capture shortcut; the Tasks tab focuses its field when it changes. */
  let captureKey = $state(0);
  /** The System view's Dev tab (Settings → Modules → System). */
  let devTools = $state(true);
  /** Live items the user wants to see in the notch. */
  const shownItems = $derived(items.filter((item) => isShown(item, notchActivities)));
  /** Tiles in the user's chosen order. */
  const visibleModules = $derived(
    notchModules
      .map((id) => modules.find((m) => m.id === id))
      .filter((m): m is (typeof modules)[number] => m !== undefined),
  );
  const current = $derived<LiveItem | undefined>(
    shownItems[rotation % Math.max(shownItems.length, 1)],
  );
  /** The compact pill's text for the current item (translated when it has a `kind`). */
  const currentText = $derived(current ? liveText(current, t.live) : null);

  function request(next: IslandState) {
    if (next !== island) void ipc.islandSetState(next);
  }

  function scheduleHide(delay: number) {
    clearTimeout(leaveTimer);
    leaveTimer = setTimeout(() => {
      if (!hovering && island === "compact") request("hidden");
    }, delay);
  }

  // ---- "A moment for you": once a day, a pill that opens a card with the thought ----
  /** The invitation pill is showing ("A little thought for today"). */
  let momentPill = $state(false);
  let dailyThought = $state(true);
  let savedThoughts = $state<string[]>([]);
  /** The preferences as last received, to save a thought without losing anything else. */
  let latestPrefs: Preferences | null = null;
  const MOMENT_PILL_MS = 9000;
  const MOMENT_KEY = "preku.moment.day";

  /** Preku's thoughts plus those of switched-on skills; one per local day. */
  const todaysPick = $derived(thoughtOn(now, thoughtPool(skills.installed)));
  const todaysThought = $derived(todaysPick.text);
  const momentSource = $derived(
    todaysPick.skill
      ? t.moment.fromSkill(todaysPick.skill, todaysPick.author)
      : todaysPick.author
        ? t.moment.by(todaysPick.author)
        : t.moment.source,
  );
  const momentGreeting = $derived.by(() => {
    const sky = skyLine(weather.state, t.moment.sky);
    return sky ? t.moment.helloSky(greetingFor(now), sky) : t.moment.hello(greetingFor(now));
  });

  function momentSeenDay(): string | null {
    try {
      return localStorage.getItem(MOMENT_KEY);
    } catch {
      return null;
    }
  }

  /** Today's moment was answered (or seen and closed): it stays away until tomorrow. */
  function finishMoment() {
    momentOpen = false;
    momentPill = false;
    try {
      localStorage.setItem(MOMENT_KEY, dayKey(new Date()));
    } catch {
      // Not remembered: at worst it is offered once more today.
    }
  }

  function openMoment() {
    momentPill = false;
    momentOpen = true;
    openModule = null;
    request("expanded");
  }

  function saveMoment() {
    if (latestPrefs) {
      const next = { ...latestPrefs, savedThoughts: saveThought(savedThoughts, todaysThought) };
      latestPrefs = next;
      savedThoughts = next.savedThoughts;
      void ipc.preferencesSet(next);
    }
  }

  /** False until the first list arrives (start-up): that one is silent. */
  let itemsLoaded = false;

  function receiveItems(next: LiveItem[]) {
    const visible = next.filter((item) => isShown(item, notchActivities));
    const previousTop = shownItems[0]?.id;
    items = next;
    // A new top item (e.g. a fresh alert) is shown immediately instead of waiting its turn.
    if (visible[0]?.id !== previousTop) rotation = 0;

    // Only activities the user kept may pop the island out.
    const fresh = visible.filter(
      (item) => isUrgent(item) && !seenUrgent.has(item.id + item.createdAt),
    );
    fresh.forEach((item) => seenUrgent.add(item.id + item.createdAt));
    // The pop-up sound: once per new reminder or alert, never for what was already there
    // when Preku started, and not on top of the timer's own finishing chime.
    // A skill nudge may bring its own sound, or ask for none.
    const audible = fresh.find((item) => item.id !== "productivity.timer-done");
    if (itemsLoaded && audible && sounds && !quiet.state.screenBusy) {
      const sound =
        audible.module === "skills" ? nudgeSound(skills.installed, audible.id) : "default";
      if (sound === "default") playPopup();
      else if (sound !== "none") playNotes(sound);
    }
    itemsLoaded = true;
    if (fresh.length > 0 && island === "hidden" && !quiet.state.screenBusy) {
      rotation = 0;
      request("compact");
      scheduleHide(POP_MS);
    }
  }

  function applyPreferences(prefs: Preferences) {
    position = prefs.islandPosition;
    notchActivities = prefs.notchActivities;
    notchModules = prefs.notchModules;
    pillRadius = prefs.pillRadius;
    devTools = prefs.devTools;
    // A hidden tab can't stay open.
    if (!devTools && openModule === "monitor" && tab === "dev") tab = "overview";
    sounds = prefs.sounds;
    dailyThought = prefs.dailyThought;
    todayOnHome = prefs.todayOnHome;
    savedThoughts = prefs.savedThoughts;
    latestPrefs = prefs;
  }

  onMount(() => {
    const cleanups: Array<() => void> = [];
    let disposed = false;

    void (async () => {
      const [state, live, pipMood, prefs, info] = await Promise.all([
        ipc.islandGetState(),
        ipc.liveItems(),
        ipc.pipMood(),
        ipc.preferencesGet(),
        ipc.appInfo(),
      ]);
      if (state.ok) island = state.value;
      const desktop = await ipc.islandOnDesktop();
      if (desktop.ok) onDesktop = desktop.value;
      if (live.ok) items = live.value;
      if (pipMood.ok) mood = pipMood.value;
      if (prefs.ok) applyPreferences(prefs.value);
      // Already open at load (rare): the first open of the day shows the moment.
      if (
        prefs.ok &&
        island === "expanded" &&
        momentDue(prefs.value.dailyThought, momentSeenDay(), new Date())
      ) {
        momentOpen = true;
      }
      // Once a day, shortly after start: an invitation in the pill (never over a game or a
      // presentation). Ignoring it is fine: the card is also there the first time the notch
      // is opened today.
      if (prefs.ok && momentDue(prefs.value.dailyThought, momentSeenDay(), new Date())) {
        setTimeout(() => {
          if (disposed || island !== "hidden" || quiet.state.screenBusy) return;
          momentPill = true;
          request("compact");
          setTimeout(() => (momentPill = false), MOMENT_PILL_MS + 500);
        }, 2500);
      }
      if (info.ok) shortcut = info.value.shortcut;

      const unlisteners = [
        await listen<IslandState>(events.islandState, (e) => {
          const opened = e.payload === "compact" && island === "hidden";
          island = e.payload;
          if (e.payload === "hidden") {
            openModule = null;
            momentPill = false;
            // Seen and closed counts as answered: no asking twice in a day.
            if (momentOpen) finishMoment();
          }
          // The first time the notch is opened today, it opens on the moment.
          if (e.payload === "expanded" && momentDue(dailyThought, momentSeenDay(), new Date())) {
            momentOpen = true;
            momentPill = false;
          }
          // Opened from the screen edge (or an alert) without the pointer entering yet:
          // close again unless the user moves onto the pill. The invitation stays a bit longer.
          if (opened && !hovering) scheduleHide(momentPill ? MOMENT_PILL_MS : EDGE_IDLE_MS);
        }),
        await listen<LiveItem[]>(events.liveItems, (e) => receiveItems(e.payload)),
        // Quick capture (its own shortcut): Tasks, with the cursor in "add a task".
        await listen("island:capture", () => {
          openModuleView("productivity", "tasks");
          captureKey += 1;
        }),
        await listen<boolean>(events.islandDesktop, (e) => (onDesktop = e.payload)),
        await listen<PipMood>(events.pipMood, (e) => (mood = e.payload)),
        await listen<Preferences>(events.preferences, (e) => applyPreferences(e.payload)),
        // Clicking anywhere else closes the panel.
        await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (!focused && island === "expanded") request("hidden");
        }),
      ];
      unlisteners.push(
        await productivity.connect(() => {
          if (sounds) playComplete();
        }),
        await monitor.connect(),
        await wellbeing.connect(),
        await weather.connect(),
        await bridge.connect(),
        await calendar.connect(),
        await quiet.connect(),
        await skills.connect(),
      );
      if (disposed) unlisteners.forEach((off) => off());
      else cleanups.push(...unlisteners);
    })();

    const clock = setInterval(() => (now = new Date()), 10_000);
    const rotate = setInterval(() => {
      rotation = nextRotation(rotation, shownItems.length);
    }, ROTATE_MS);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && island === "expanded") request("hidden");
    };
    window.addEventListener("keydown", onKey);

    return () => {
      disposed = true;
      cleanups.forEach((off) => off());
      clearInterval(clock);
      clearInterval(rotate);
      clearTimeout(leaveTimer);
      window.removeEventListener("keydown", onKey);
    };
  });

  function onHandleDown(e: PointerEvent) {
    dragStartY = e.clientY;
    dragY = 0;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onHandleMove(e: PointerEvent) {
    if (dragStartY !== null) dragY = dragOffset(e.clientY - dragStartY);
  }

  function onHandleUp(e: PointerEvent) {
    if (dragStartY === null) return;
    const dy = e.clientY - dragStartY;
    dragStartY = null;
    dragY = 0;
    if (classifyHandleRelease(dy) === "hide") request("hidden");
  }

  function onHandleCancel() {
    dragStartY = null;
    dragY = 0;
  }

  // Compact pill: pull it down to open the panel (the reverse of pushing the handle up).
  let pullStartY: number | null = null;

  function onPillDown(e: PointerEvent) {
    pullStartY = e.clientY;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPillUp(e: PointerEvent) {
    if (pullStartY === null) return;
    const dy = e.clientY - pullStartY;
    pullStartY = null;
    if (isPullDown(dy)) request("expanded");
  }

  /** Keyboard activation (Enter/Space) arrives as a click with `detail === 0`. */
  function onHandleKeyClick(e: MouseEvent) {
    if (e.detail === 0) request("hidden");
  }

  /** Scroll up over the open panel closes it; scroll down over the pill opens it. */
  function onWheel(e: WheelEvent) {
    if (island === "hidden") return;
    // Scrolling a list (tasks, clipboard) scrolls the list, not the island.
    if (canScrollWithin(e.target as unknown as ScrollBox, e.currentTarget, e.deltaY)) return;
    wheelSum += e.deltaY;
    clearTimeout(wheelReset);
    wheelReset = setTimeout(() => (wheelSum = 0), 300);
    if (island === "expanded" && wheelSum <= -WHEEL_THRESHOLD) {
      wheelSum = 0;
      request("hidden");
    } else if (island === "compact" && wheelSum >= WHEEL_THRESHOLD) {
      wheelSum = 0;
      request("expanded");
    }
  }

  function onEnter() {
    hovering = true;
    clearTimeout(leaveTimer);
    if (island === "hidden") request("compact");
  }

  function onLeave() {
    hovering = false;
    if (island === "compact") scheduleHide(LEAVE_DELAY_MS);
  }
</script>

<div
  class="stage"
  data-position={position}
  role="presentation"
  onmouseenter={onEnter}
  onmouseleave={onLeave}
>
  <div
    class="island"
    class:dragging={dragY !== 0}
    data-state={island}
    data-desktop={onDesktop}
    data-nudge={companion.prompt && companionShown && !companion.session
      ? isNudge(companion.prompt.kind)
      : false}
    data-thought={momentPill}
    style:--panel-h="{panelHeight}px"
    style:--pill-radius="{pillRadius}px"
    style:--tile-count={visibleModules.length}
    role="region"
    aria-label={t.islandRegion}
    style:transform={dragY !== 0 ? `translateY(${dragY}px)` : undefined}
    onwheel={onWheel}
  >
    {#if droppingFiles}
      <div class="file-drop" class:off={!dropPhone} aria-live="polite">
        {dropPhone ? t.bridge.dropToSend(dropPhone) : t.bridge.phoneOffline}
      </div>
    {/if}
    {#if island === "compact" && momentPill}
      <!-- The daily invitation: click to open "A moment for you"; it leaves by itself. -->
      <div class="pill prompt-pill nudge" style:--accent="var(--brand)">
        <button class="prompt-open" onclick={openMoment}>
          <span class="pill-mascot"><Pip size={24} mood="happy" fps={30} label={t.pipLabel} /></span
          >
          <span class="prompt-title" aria-live="polite">{greetingFor(now)} · {t.moment.pill}</span>
        </button>
        <button class="chip close" onclick={finishMoment} aria-label={t.moment.dismiss}>✕</button>
      </div>
    {:else if island === "compact" && companion.prompt && companionShown && !companion.session && isNudge(companion.prompt.kind)}
      <!-- A drive-by nudge: Pip says it, one tap answers it, and it leaves by itself. -->
      {@const prompt = companion.prompt}
      <div class="pill prompt-pill nudge" style:--accent="var(--mod-wellbeing)">
        <button class="prompt-open" onclick={() => request("expanded")} aria-label={t.openIsland}>
          <span class="pill-mascot"
            ><Pip size={24} mood={displayMood} fps={30} label={t.pipLabel} /></span
          >
          <span class="prompt-title" aria-live="polite">{nudgeLine(prompt)}</span>
        </button>
        <button
          class="chip primary"
          onclick={() => {
            void companion.take(prompt.kind);
            request("hidden");
          }}
        >
          {prompt.kind === "water"
            ? t.wellbeing.addGlass(wellbeing.state?.settings.glassMl ?? 250)
            : t.companion.gotIt}
        </button>
        <button
          class="chip close"
          onclick={() => {
            void companion.finish(prompt.kind, false);
            request("hidden");
          }}
          aria-label={t.companion.notNow}
          title={t.companion.notNow}>✕</button
        >
      </div>
    {:else if island === "compact" && companion.prompt && companionShown && !companion.session}
      {@const prompt = companion.prompt}
      <div class="pill prompt-pill" style:--accent="var(--mod-wellbeing)">
        <button class="prompt-open" onclick={() => request("expanded")} aria-label={t.openIsland}>
          <span class="pill-mascot"
            ><Pip size={24} mood={displayMood} fps={30} label={t.pipLabel} /></span
          >
          <span class="prompt-title">{t.companion.short[prompt.kind]}</span>
        </button>
        <button
          class="chip primary"
          onclick={() => {
            void companion.take(prompt.kind);
            request("expanded");
          }}
        >
          {prompt.kind === "moodCheckIn"
            ? t.companion.answer
            : prompt.kind === "posture"
              ? t.companion.take.posture
              : t.companion.takeShort}
        </button>
        <button
          class="chip"
          onclick={() => {
            void companion.later(prompt);
            request("hidden");
          }}
        >
          {t.companion.later}
        </button>
      </div>
    {:else if island === "compact"}
      <button
        class="pill"
        onclick={() => request("expanded")}
        onpointerdown={onPillDown}
        onpointerup={onPillUp}
        onpointercancel={() => (pullStartY = null)}
        aria-label={t.openIsland}
      >
        <span class="pill-mascot"
          ><Pip size={24} mood={displayMood} fps={30} label={t.pipLabel} /></span
        >
        {#if current}
          {#key current.id + current.createdAt}
            <span class="live" style:--accent={accentFor(current.module)}>
              <span class="live-title">
                {#if current.id === "productivity.media"}
                  <!-- Song titles scroll so the whole name can be read. -->
                  <Marquee text={currentText?.title ?? ""} speed={24} />
                {:else}
                  {currentText?.title}
                {/if}
              </span>
              {#if current.progress !== null}
                <svg class="ring" viewBox="0 0 20 20" aria-hidden="true">
                  <circle cx="10" cy="10" r="8" class="track" />
                  <circle
                    cx="10"
                    cy="10"
                    r="8"
                    class="value"
                    pathLength="100"
                    stroke-dasharray="{current.progress * 100} 100"
                  />
                </svg>
              {:else if currentText?.subtitle}
                <span class="live-sub">{currentText.subtitle}</span>
              {/if}
            </span>
          {/key}
        {:else}
          <span class="identity"
            ><span class="name">{t.appName}</span><span class="pill-hint">{t.hoverHint}</span></span
          >
          {#if weather.state && notchActivities.weather !== false}
            <span class="pill-weather" title={weather.state.place}>
              {weatherIcon(weather.state.code, weather.state.isDay)}
              {formatTemp(weather.state.temperature)}
            </span>
          {/if}
          <span class="time">{time}</span>
        {/if}
      </button>
    {:else if island === "expanded"}
      <section class="panel">
        {#if openModule === null && momentOpen}
          <MomentCard
            greeting={momentGreeting}
            thought={todaysThought}
            source={momentSource}
            saved={savedThoughts.includes(todaysThought)}
            onsave={saveMoment}
            ondismiss={finishMoment}
          />
        {:else if openModule === null}
          <header>
            <button
              class="pet"
              class:away={stage || companionShown}
              onclick={() => bigPip?.poke()}
              aria-label={t.petPip}
            >
              <Pip bind:this={bigPip} size={headerPip} mood={displayMood} label={t.pipLabel} />
            </button>
            <div class="headline">
              <span class="greeting">{greetingFor(now)}</span>
              <span class="clock">{time}</span>
            </div>
            {#if weather.state}
              {@const w = weather.state}
              <div class="weather" title={t.weather.conditions[condition(w.code)]}>
                <span class="weather-icon" aria-hidden="true">{weatherIcon(w.code, w.isDay)}</span>
                <span class="weather-text">
                  <span class="weather-temp">{formatTemp(w.temperature)}</span>
                  <span class="weather-meta">
                    {w.place} · {t.weather.highLow(formatTemp(w.high), formatTemp(w.low))}
                  </span>
                </span>
              </div>
            {/if}
            <QuietButton now={now.getTime()} />
            <button
              class="gear"
              onclick={() => void ipc.openSettings()}
              aria-label={t.openSettings}
            >
              <Icon name="settings" size={20} />
            </button>
          </header>

          <div class="content">
            <NowPlayingCard staged={stage} onstage={() => (stage = !stage)} />
            {#if todayShown && !stage && !companionShown}
              <TodayCard
                now={now.getTime()}
                onopen={(next) => openModuleView("productivity", next)}
              />
            {/if}
            {#if companionShown}
              <CompanionCard />
            {:else if stage}
              <DanceStage
                playing={productivity.media?.playing ?? false}
                onclose={() => (stage = false)}
              />
            {:else}
              <ul class="modules">
                {#each visibleModules as module (module.id)}
                  <li style:--accent={accentFor(module.id)}>
                    {#if module.id === "productivity" || module.id === "monitor" || module.id === "wellbeing"}
                      <button class="tile" onclick={() => openModuleView(module.id)}>
                        <span class="module-icon mascot"
                          ><ModuleMascot module={module.id} size={36} /></span
                        >
                        <span class="module-name">{t.modules[module.id]}</span>
                        <span class="module-status">
                          {module.id === "productivity"
                            ? productivityStatus
                            : module.id === "monitor"
                              ? monitorStatus
                              : wellbeingStatus}
                        </span>
                      </button>
                    {:else if module.id === "bridge"}
                      <button class="tile" onclick={() => openModuleSettings("bridge")}>
                        <span class="module-icon"><Icon name={module.id} /></span>
                        <span class="module-name">{t.modules[module.id]}</span>
                        <span class="module-status">{phoneStatus}</span>
                      </button>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {:else}
          <header>
            <button class="back" onclick={() => (openModule = null)} aria-label={t.backHome}>
              <Icon name="back" size={18} />
            </button>
            <div
              class="tabs"
              role="tablist"
              aria-label={t.modules[openModule]}
              style:--accent={accentFor(openModule)}
            >
              {#each moduleTabs[openModule].filter((id) => id !== "dev" || devTools) as id (id)}
                <button
                  role="tab"
                  id="tab-{id}"
                  aria-selected={tab === id}
                  aria-controls="tabpanel"
                  class:active={tab === id}
                  onclick={() => openModule && openModuleView(openModule, id)}
                >
                  <Icon name={tabIcons[id] ?? id} size={16} />
                  <span>{tabLabels[id]}</span>
                </button>
              {/each}
            </div>
            <button
              class="gear"
              onclick={() => void ipc.openSettings()}
              aria-label={t.openSettings}
            >
              <Icon name="settings" size={20} />
            </button>
          </header>

          <div
            class="content"
            id="tabpanel"
            role="tabpanel"
            aria-labelledby="tab-{tab}"
            style:--accent={accentFor(openModule)}
          >
            {#if openModule === "productivity" && productivity.error}
              <p class="error" role="alert">{productivity.error}</p>
            {:else if openModule === "monitor" && monitor.error}
              <p class="error" role="alert">{monitor.error}</p>
            {:else if openModule === "wellbeing" && wellbeing.error}
              <p class="error" role="alert">{wellbeing.error}</p>
            {/if}
            {#if tab === "focus"}
              <FocusTab />
            {:else if tab === "tasks"}
              {#if !productivity.error}
                <TodayCard
                  now={now.getTime()}
                  onopen={(next) => openModuleView("productivity", next)}
                />
              {/if}
              <TasksTab onfocus={() => openModuleView("productivity", "focus")} {captureKey} />
            {:else if tab === "clipboard"}
              <ClipboardTab />
            {:else if tab === "messages"}
              <MessagesTab onOpenPage={() => openModuleSettings("bridge")} />
            {:else if tab === "overview"}
              <OverviewTab />
            {:else if tab === "processes"}
              <ProcessesTab />
            {:else if tab === "dev"}
              <DevTab />
            {:else if tab === "alerts"}
              <AlertsTab {items} onOpenPage={() => openModuleSettings("monitor")} />
            {:else if tab === "today"}
              <TodayTab
                onOpenTab={(next) => openModuleView("wellbeing", next)}
                onOpenPage={() => openModuleSettings("wellbeing")}
              />
            {:else if tab === "habits"}
              <HabitsTab onOpenPage={() => openModuleSettings("wellbeing")} />
            {/if}
          </div>
        {/if}

        <footer>
          <button
            class="handle"
            aria-label={t.pushToHide}
            title={t.handleTooltip(shortcut)}
            onpointerdown={onHandleDown}
            onpointermove={onHandleMove}
            onpointerup={onHandleUp}
            onpointercancel={onHandleCancel}
            onclick={onHandleKeyClick}
          >
            <svg class="chevron" viewBox="0 0 24 12" aria-hidden="true">
              <path d="M6 9l6-5 6 5" />
            </svg>
            <span class="grip" aria-hidden="true"></span>
          </button>
        </footer>
      </section>
    {/if}
  </div>
</div>

<style>
  :global(html[data-window="island"]),
  :global(html[data-window="island"] body) {
    background: transparent;
    overflow: hidden;
    user-select: none;
  }

  .stage {
    position: fixed;
    inset: 0;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }

  /* With the island pinned left/right, grow away from the screen edge. */
  .stage[data-position="left"] {
    justify-content: flex-start;
    padding-left: var(--space-3);
  }

  .stage[data-position="right"] {
    justify-content: flex-end;
    padding-right: var(--space-3);
  }

  .island {
    position: relative;
    background: linear-gradient(180deg, var(--island-bg), var(--island-bg-raised));
    color: var(--island-text);
    border: 1px solid var(--island-border);
    border-top: none;
    overflow: hidden;
    /* Hangs from the top edge like a notch: square top, round bottom. */
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
    transition:
      width var(--dur-base) var(--ease-out),
      height var(--dur-base) var(--ease-out),
      border-radius var(--dur-base) var(--ease-out),
      transform var(--dur-base) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }

  .island[data-state="hidden"] {
    width: 180px;
    height: 4px;
    /* A quiet metallic edge remains visible against light and dark apps. */
    background: linear-gradient(
      90deg,
      var(--island-text-muted),
      var(--island-text),
      var(--island-text-muted)
    );
    border: none;
    opacity: 0.7;
    box-shadow: none;
  }

  /* Over other apps the line disappears; hovering the top edge still opens the island. */
  .island[data-state="hidden"][data-desktop="false"] {
    opacity: 0;
  }

  .island[data-state="compact"] {
    width: 300px;
    height: 36px;
    /* User-chosen corner radius (Settings → Notch). */
    border-radius: 0 0 var(--pill-radius, 18px) var(--pill-radius, 18px);
    box-shadow: var(--shadow-island);
  }

  /* Room for a whole sentence when Pip drops by. */
  .island[data-state="compact"][data-nudge="true"] {
    width: 400px;
  }

  /* The daily invitation carries a greeting too: a little wider than a nudge. */
  .island[data-state="compact"][data-thought="true"] {
    width: 420px;
  }

  .island[data-state="expanded"] {
    width: 480px;
    height: var(--panel-h, 360px);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
    box-shadow: var(--shadow-island);
  }

  .pill,
  .panel {
    --focus-ring: 0 0 0 2px var(--island-bg), 0 0 0 4px var(--brand);
    animation: reveal var(--dur-base) var(--ease-out) 40ms both;
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  button {
    background: none;
    border: none;
    padding: 0;
    color: inherit;
    cursor: pointer;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: 100%;
    padding: 0 var(--space-3);
    border-radius: inherit;
    font: var(--text-pill);
  }

  /* A companion prompt in the pill: open · Break · Later. */
  .prompt-pill {
    gap: var(--space-1);
    padding-right: var(--space-2);
  }

  .prompt-open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 100%;
    font: var(--text-pill);
    color: var(--island-text);
    text-align: left;
  }

  .chip.close {
    padding: 3px 7px;
  }

  .prompt-title {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .chip {
    flex: none;
    padding: 3px 10px;
    font: var(--text-pill);
    color: var(--island-text-muted);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .chip:hover {
    color: var(--island-text);
  }

  .chip.primary {
    color: #10210f;
    background: var(--accent);
    border-color: var(--accent);
  }

  .identity {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    flex: 1;
    min-width: 0;
  }

  .name {
    font: var(--text-pill);
  }

  .pill-hint {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .pill-mascot {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    background: color-mix(in srgb, var(--pip-body) 10%, var(--island-bg));
    border-radius: var(--radius-pill);
  }

  .island::after {
    content: "";
    position: absolute;
    inset: auto var(--space-4) 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--island-border), transparent);
    pointer-events: none;
  }

  .island[data-state="hidden"]::after {
    display: none;
  }

  .pill:focus-visible {
    outline: 2px solid var(--brand);
    outline-offset: -3px;
    box-shadow: none;
  }

  .live {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    animation: slide-in var(--dur-slow) var(--ease-out) both;
  }

  .live-title {
    flex: 1;
    min-width: 0;
    text-align: left;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .live-sub {
    flex: none;
    max-width: 45%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: var(--text-caption);
    color: var(--accent);
  }

  .ring {
    flex: none;
    width: 20px;
    height: 20px;
    transform: rotate(-90deg);
  }

  .ring circle {
    fill: none;
    stroke-width: 2.5;
  }

  .ring .track {
    stroke: var(--island-bg-raised);
  }

  .ring .value {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dasharray var(--dur-slow) var(--ease-out);
  }

  .time {
    font: var(--text-mono);
    color: var(--island-text-muted);
  }

  .panel {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    /* minmax(0, …) so long text (clipboard, tasks) truncates instead of widening the panel. */
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-3);
    height: 100%;
    padding: var(--space-4) var(--space-5) 0;
  }

  /* Explicit rows so the tiles stay in the flexible row with or without a live item. */
  .panel > header {
    grid-row: 1;
  }

  .content {
    grid-row: 2;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* Productivity views fill the remaining height. */
  .content > :global(:last-child:not(.error)) {
    flex: 1;
    min-height: 0;
  }

  .panel > footer {
    grid-row: 3;
  }

  .tabs {
    flex: 1;
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .tabs button {
    /* Share the bar by label length, so four tabs with long names still fit. */
    flex: 1 1 auto;
    min-width: 0;
    padding: 0 var(--space-2);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    height: 30px;
    font: var(--text-pill);
    color: var(--island-text-muted);
    border-radius: var(--radius-pill);
    transition:
      color var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out);
  }

  .tabs button:hover {
    color: var(--island-text);
  }

  .tabs button.active {
    color: var(--island-bg);
    background: var(--accent);
  }

  .error {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font: var(--text-caption);
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border-radius: var(--radius-sm);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding-bottom: var(--space-3);
    border-bottom: 1px solid var(--island-border);
  }

  .pet {
    margin: -8px 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  /* Pip has gone down to the dance stage. */
  .pet.away {
    opacity: 0;
    pointer-events: none;
  }

  .pet,
  .gear,
  .back {
    border-radius: var(--radius-md);
  }

  .headline {
    /* Keeps its width: the weather card gives way first when the header gets busy. */
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
    white-space: nowrap;
  }

  .greeting {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .clock {
    font: var(--text-title);
    font-family: var(--font-mono);
  }

  .back {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--island-text-muted);
  }

  .back:hover {
    color: var(--island-text);
    background: var(--island-bg-raised);
  }

  /* All module tiles in one row keeps the panel short (user preference). */
  .modules {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(var(--tile-count, 4), minmax(0, 1fr));
    gap: var(--space-2);
    min-height: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .modules li {
    display: flex;
    min-height: 0;
  }

  .tile {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--space-2) var(--space-3) var(--space-3);
    text-align: left;
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  button.tile:hover {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }

  button.tile:active {
    transform: scale(0.98);
  }

  .module-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin-bottom: var(--space-1);
    border-radius: var(--radius-sm);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  /* Pip in a role replaces the line icon: no tinted square, a little larger. */
  .module-icon.mascot {
    width: 36px;
    height: 36px;
    margin-bottom: 0;
    background: none;
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .tile:hover .module-icon.mascot {
    transform: translateY(-2px) rotate(-4deg);
  }

  @media (prefers-reduced-motion: reduce) {
    .tile:hover .module-icon.mascot {
      transform: none;
    }
  }

  .module-name {
    font: var(--text-body);
    font-weight: 700;
  }

  .module-status {
    /* Two lines at most, so live status like "CPU 16% · RAM 59%" fits a narrow tile. */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  button.tile .module-status {
    color: var(--accent);
  }

  .gear {
    align-self: center;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--island-text-muted);
    transition: color var(--dur-fast) var(--ease-out);
  }

  .gear:hover {
    background: var(--island-bg-raised);
    color: var(--island-text);
  }

  footer {
    display: flex;
    justify-content: center;
    margin: 0 calc(var(--space-5) * -1);
  }

  /* Pull-tab along the bottom edge: click or push up to hide. */
  .handle {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    width: 100%;
    height: 22px;
    padding-bottom: var(--space-2);
    color: var(--island-text-muted);
    cursor: grab;
    touch-action: none;
  }

  .island.dragging .handle {
    cursor: grabbing;
  }

  .grip {
    width: 36px;
    height: 4px;
    border-radius: var(--radius-pill);
    background: currentColor;
    opacity: 0.5;
    transition:
      opacity var(--dur-fast) var(--ease-out),
      width var(--dur-fast) var(--ease-out);
  }

  .chevron {
    width: 14px;
    height: 7px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0;
    transform: translateY(3px);
    transition:
      opacity var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  .handle:hover,
  .handle:focus-visible,
  .island.dragging .handle {
    color: var(--island-text);
  }

  .handle:hover .grip,
  .island.dragging .grip {
    opacity: 0.9;
    width: 44px;
  }

  .handle:hover .chevron,
  .handle:focus-visible .chevron,
  .island.dragging .chevron {
    opacity: 1;
    transform: none;
  }

  /* Follow the pointer 1:1 while dragging; animate only when released. */
  .island.dragging {
    transition: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .island {
      transition: opacity var(--dur-fast) linear;
    }

    .live,
    .pill,
    .panel {
      animation: fade-in var(--dur-fast) linear both;
    }
  }

  /* ---- Weather (opt-in) ---- */
  .pill-weather {
    flex: none;
    font: var(--text-caption);
    color: var(--island-text-muted);
    white-space: nowrap;
  }

  .weather {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
    flex: 0 1 auto;
    min-width: 0;
    max-width: 50%;
  }

  .weather-icon {
    font-size: 22px;
    line-height: 1;
  }

  .weather-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .weather-temp {
    font: var(--text-heading);
    font-family: var(--font-mono);
  }

  .weather-meta {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  /* Files dragged over the island. */
  .file-drop {
    position: absolute;
    inset: 4px;
    z-index: 5;
    display: grid;
    place-items: center;
    padding: 0 var(--space-3);
    text-align: center;
    font: var(--text-pill);
    color: var(--island-text);
    background: color-mix(in srgb, var(--mod-bridge) 35%, var(--island-bg));
    border: 1.5px dashed var(--mod-bridge);
    border-radius: inherit;
    pointer-events: none;
  }

  .file-drop.off {
    background: var(--island-bg-raised);
    border-color: var(--island-border);
    color: var(--island-text-muted);
  }
</style>
