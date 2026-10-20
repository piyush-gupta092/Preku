<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Icon from "../../lib/Icon.svelte";
  import { events, ipc } from "../../lib/ipc";
  import { formatShortcut } from "../../lib/i18n/en";
  import { t } from "../../lib/i18n";
  import type { AppInfo } from "../../lib/types/generated/AppInfo";
  import type { IslandPosition } from "../../lib/types/generated/IslandPosition";
  import type { ModuleInfo } from "../../lib/types/generated/ModuleInfo";
  import type { MonitorInfo } from "../../lib/types/generated/MonitorInfo";
  import type { Preferences } from "../../lib/types/generated/Preferences";
  import type { Theme } from "../../lib/types/generated/Theme";
  import type { TemperatureUnit } from "../../lib/types/generated/TemperatureUnit";
  import type { WeatherPlace } from "../../lib/types/generated/WeatherPlace";
  import Pip from "../../pip/Pip.svelte";
  import { ACTIVITIES, accentFor } from "../island/live";
  import { GALLERY } from "../../pip/expressions";
  import ModuleMascot from "../../lib/ModuleMascot.svelte";
  import BridgePanel from "./BridgePanel.svelte";
  import CalendarPanel from "./CalendarPanel.svelte";
  import DataPanel from "./DataPanel.svelte";
  import SkillsPanel from "./SkillsPanel.svelte";
  import SystemPanel from "./SystemPanel.svelte";
  import TodayPage from "./TodayPage.svelte";
  import Welcome from "./Welcome.svelte";
  import WellbeingPanel from "./WellbeingPanel.svelte";
  import { MASCOT_STYLES } from "../../pip/bodies";
  import { MASCOT_COLORS } from "../../pip/palette";
  import { mascot } from "../../lib/stores/mascot.svelte";
  import type { MascotStyle } from "../../lib/types/generated/MascotStyle";
  import type { PipMood } from "../../pip/renderer";

  const pages = [
    { id: "today", icon: "tasks" },
    { id: "general", icon: "settings" },
    { id: "notch", icon: "notch" },
    { id: "modules", icon: "grid" },
    { id: "skills", icon: "skills" },
    { id: "about", icon: "info" },
  ] as const;
  type Page = (typeof pages)[number]["id"];

  const positions: IslandPosition[] = ["left", "center", "right"];
  const themes: Theme[] = ["system", "light", "dark"];
  /** Every module tile, in the order the notch shows them. */
  const notchTiles = ["productivity", "monitor", "wellbeing", "bridge"];
  /** Module cards in this order. */
  const moduleOrder = ["productivity", "monitor", "wellbeing", "weather", "bridge"];

  let prefs = $state<Preferences | null>(null);
  let monitors = $state<MonitorInfo[]>([]);
  let modules = $state<ModuleInfo[]>([]);
  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  /** Live value while dragging the radius slider; saved when released. */
  let radiusDraft = $state<number | null>(null);
  let now = $state(new Date());
  /** Expression gallery: the one shown large, and the thumbnail under the pointer. */
  let expression = $state<PipMood>("happy");
  let hovered = $state<PipMood | null>(null);
  let hoveredBody = $state<MascotStyle | null>(null);

  /** Updates every Pip at once (before the save round-trips through the backend). */
  function setMascot(change: Partial<Preferences>) {
    if (!prefs) return;
    mascot.apply({ ...prefs, ...change });
    void save(change);
  }

  const PAGE_KEY = "preku.settings.page";
  function savedPage(): Page {
    try {
      const value = localStorage.getItem(PAGE_KEY);
      return pages.some((p) => p.id === value) ? (value as Page) : "notch";
    } catch {
      return "notch";
    }
  }
  let page = $state<Page>(savedPage());
  onMount(() => {
    const follow = (e: StorageEvent) => {
      if (e.key === PAGE_KEY && pages.some((p) => p.id === e.newValue)) page = e.newValue as Page;
      if (e.key === "preku.settings.module" && e.newValue) moduleView = e.newValue;
    };
    window.addEventListener("storage", follow);
    return () => window.removeEventListener("storage", follow);
  });
  $effect(() => {
    try {
      localStorage.setItem(PAGE_KEY, page);
    } catch {
      // Storage unavailable: the page just isn't remembered.
    }
  });

  /** One module's page (sidebar sub-item), or null for the overview of all modules. */
  const MODULE_KEY = "preku.settings.module";
  function savedModule(): string | null {
    try {
      return localStorage.getItem(MODULE_KEY) || null;
    } catch {
      return null;
    }
  }
  let moduleView = $state<string | null>(savedModule());
  // The list starts open when Settings reopens on a Modules page.
  let modulesOpen = $state(savedPage() === "modules");
  $effect(() => {
    try {
      localStorage.setItem(MODULE_KEY, moduleView ?? "");
    } catch {
      // Storage unavailable: the module just isn't remembered.
    }
  });

  /** The first-run welcome is showing instead of the pages. */
  const welcoming = $derived(prefs !== null && !prefs.welcomed);

  /**
   * Using the sidebar while the welcome shows means "skip it": the welcome must never trap
   * someone on one screen (it used to replace every page until Start or Skip was pressed).
   */
  function leaveWelcome() {
    if (welcoming) void save({ welcomed: true });
  }

  function openPage(next: Page) {
    leaveWelcome();
    page = next;
  }

  function openModules() {
    leaveWelcome();
    // A second click on the open overview folds the list away again.
    if (page === "modules" && moduleView === null && modulesOpen) {
      modulesOpen = false;
      return;
    }
    page = "modules";
    moduleView = null;
    modulesOpen = true;
  }

  function openModule(id: string) {
    leaveWelcome();
    page = "modules";
    moduleView = id;
    modulesOpen = true;
  }

  const radius = $derived(radiusDraft ?? prefs?.pillRadius ?? 18);
  const clock = $derived(now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }));
  const moduleCards = $derived([
    ...moduleOrder,
    ...modules.map((m) => m.id).filter((id) => !moduleOrder.includes(id)),
  ]);
  /** Modules that exist (plus the upcoming phone bridge), for the sidebar sub-list. */
  const moduleNav = $derived(
    moduleCards.filter((id) => id === "bridge" || modules.some((m) => m.id === id)),
  );
  const visibleModules = $derived(
    moduleView && moduleNav.includes(moduleView) ? [moduleView] : moduleCards,
  );
  const singleModule = $derived(
    page === "modules" && visibleModules.length === 1 ? visibleModules[0] : null,
  );
  const shortcutKeys = $derived(info?.shortcut ? formatShortcut(info.shortcut).split("+") : []);

  function applyTheme(theme: Theme) {
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
  }

  async function refreshModules() {
    const result = await ipc.modulesList();
    if (result.ok) modules = result.value;
  }

  async function save(change: Partial<Preferences>) {
    if (!prefs) return;
    const next = { ...prefs, ...change };
    prefs = next;
    applyTheme(next.theme);
    const result = await ipc.preferencesSet(next);
    if (result.ok) {
      error = null;
    } else {
      error = t.settings.saveFailed(result.error.message);
      const fresh = await ipc.preferencesGet();
      if (fresh.ok) {
        prefs = fresh.value;
        applyTheme(fresh.value.theme);
      }
    }
    await refreshModules();
  }

  // ---- Weather place picker ----
  const units: TemperatureUnit[] = ["celsius", "fahrenheit"];
  let weatherQuery = $state("");
  let weatherResults = $state<WeatherPlace[]>([]);
  let weatherSearching = $state(false);
  let changingPlace = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  /** Searches as the user types, at most every 350 ms. */
  function searchPlaces(query: string) {
    clearTimeout(searchTimer);
    if (query.trim().length < 2) {
      weatherResults = [];
      weatherSearching = false;
      return;
    }
    weatherSearching = true;
    searchTimer = setTimeout(async () => {
      const result = await ipc.weatherSearch(query);
      weatherSearching = false;
      weatherResults = result.ok ? result.value : [];
      if (!result.ok) error = result.error.message;
    }, 350);
  }

  async function setWeather(change: Partial<Preferences>) {
    await save(change);
    void ipc.weatherRefresh();
  }

  function choosePlace(place: WeatherPlace) {
    weatherQuery = "";
    weatherResults = [];
    changingPlace = false;
    void setWeather({ weatherPlace: place });
  }

  function toggleTile(id: string) {
    if (!prefs) return;
    const current = prefs.notchModules;
    const on = current.includes(id);
    if (on && current.length === 1) return;
    void save({
      notchModules: notchTiles.filter((tile) => (tile === id ? !on : current.includes(tile))),
    });
  }

  function setActivity(key: string, shown: boolean) {
    if (!prefs) return;
    void save({ notchActivities: { ...prefs.notchActivities, [key]: shown } });
  }

  function setModule(id: string, enabled: boolean) {
    if (!prefs) return;
    void save({ modules: { ...prefs.modules, [id]: enabled } });
  }

  onMount(() => {
    let off: (() => void) | undefined;
    let disposed = false;
    const tick = setInterval(() => (now = new Date()), 30_000);

    void (async () => {
      const [p, m, i] = await Promise.all([
        ipc.preferencesGet(),
        ipc.monitorsList(),
        ipc.appInfo(),
      ]);
      if (p.ok) {
        prefs = p.value;
        applyTheme(p.value.theme);
      } else {
        error = p.error.message;
      }
      if (m.ok) monitors = m.value;
      if (i.ok) info = i.value;
      await refreshModules();

      // Changes made elsewhere (e.g. tray Pause) show up here too.
      const unlisten = await listen<Preferences>(events.preferences, (e) => {
        prefs = e.payload;
        applyTheme(e.payload.theme);
        void refreshModules();
      });
      if (disposed) unlisten();
      else off = unlisten;
    })();

    return () => {
      disposed = true;
      clearInterval(tick);
      off?.();
    };
  });
</script>

<div class="layout">
  <nav aria-label={t.settings.title}>
    <div class="brand">
      <Pip size={72} label={t.pipLabel} />
      <span class="wordmark">{t.appName}</span>
    </div>
    <ul>
      {#each pages as p (p.id)}
        <li>
          {#if p.id === "modules"}
            <button
              class:active={!welcoming && page === "modules" && !singleModule}
              class:parent={!welcoming && page === "modules" && singleModule}
              aria-current={!welcoming && page === "modules" && !singleModule ? "page" : undefined}
              aria-expanded={modulesOpen}
              aria-controls="module-nav"
              onclick={openModules}
            >
              <Icon name={p.icon} size={20} />
              {t.settings.nav[p.id]}
              <span class="chevron" class:open={modulesOpen}><Icon name="down" size={16} /></span>
            </button>
            <div class="subnav" class:open={modulesOpen} id="module-nav">
              <ul inert={!modulesOpen}>
                {#each moduleNav as id (id)}
                  {@const module = modules.find((m) => m.id === id)}
                  <li>
                    <button
                      class="subitem"
                      class:active={singleModule === id}
                      class:off={!module?.enabled}
                      aria-current={singleModule === id ? "page" : undefined}
                      style:--accent={accentFor(id)}
                      onclick={() => openModule(id)}
                    >
                      <span class="sub-icon"><Icon name={id} size={14} /></span>
                      <span class="sub-name">{t.modules[id] ?? id}</span>
                      {#if !module}
                        <span class="sub-tag">{t.settings.soon}</span>
                      {:else if !module.enabled}
                        <span class="sub-tag">{t.settings.moduleOff}</span>
                      {/if}
                    </button>
                  </li>
                {/each}
              </ul>
            </div>
          {:else}
            <button
              class:active={!welcoming && page === p.id}
              aria-current={!welcoming && page === p.id ? "page" : undefined}
              onclick={() => openPage(p.id)}
            >
              <Icon name={p.icon} size={20} />
              {t.settings.nav[p.id]}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="tagline">{t.settings.subtitle}</p>
  </nav>

  <main>
    <!-- The welcome brings its own title; a page title above it looked like a broken page. -->
    <header class:hidden={welcoming}>
      <div>
        {#if singleModule}
          <button class="crumb" onclick={openModules}>
            <Icon name="back" size={14} />
            {t.settings.nav.modules}
          </button>
          <h1>{t.modules[singleModule] ?? singleModule}</h1>
          <p class="subtitle">{t.settings.moduleSubtitle}</p>
        {:else}
          <h1>{t.settings.pages[page]?.title}</h1>
          <p class="subtitle">{t.settings.pages[page]?.subtitle}</p>
        {/if}
      </div>
      <span class="autosave">{t.settings.autosave}</span>
    </header>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    {#if !prefs}
      <p class="muted" role="status">{t.settings.loading}</p>
    {:else if welcoming}
      <Welcome
        modules={Object.fromEntries(modules.map((m) => [m.id, m.enabled]))}
        shortcut={info?.shortcut ? formatShortcut(info.shortcut) : null}
        ondone={(chosen) =>
          save({
            welcomed: true,
            ...(chosen ? { modules: { ...prefs?.modules, ...chosen } } : {}),
          })}
      />
    {:else if page === "today"}
      <TodayPage />
    {:else if page === "skills"}
      <section class="card">
        <SkillsPanel
          nudgesShown={prefs?.notchActivities.skillNudges !== false}
          onnudges={(shown) => setActivity("skillNudges", shown)}
        />
      </section>
    {:else if page === "general"}
      <section class="card">
        <label class="row">
          <span>{t.settings.launchAtLogin}</span>
          <input
            type="checkbox"
            role="switch"
            checked={prefs.launchAtLogin}
            onchange={(e) => save({ launchAtLogin: e.currentTarget.checked })}
          />
        </label>
        <label class="row">
          <span>
            {t.settings.pause}
            <small>{t.settings.pauseHint}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={prefs.paused}
            onchange={(e) => save({ paused: e.currentTarget.checked })}
          />
        </label>
        <label class="row">
          <span>
            {t.settings.dailyThought}
            <small>{t.settings.dailyThoughtHint}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={prefs.dailyThought}
            onchange={(e) => save({ dailyThought: e.currentTarget.checked })}
          />
        </label>
        <label class="row">
          <span>
            {t.settings.todayOnHome}
            <small>{t.settings.todayOnHomeHint}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={prefs.todayOnHome}
            onchange={(e) => save({ todayOnHome: e.currentTarget.checked })}
          />
        </label>
        <label class="row">
          <span>
            {t.settings.sounds}
            <small>{t.settings.soundsHint}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={prefs.sounds}
            onchange={(e) => save({ sounds: e.currentTarget.checked })}
          />
        </label>
        <div class="row">
          <span>{t.settings.theme}</span>
          <div class="segmented" role="radiogroup" aria-label={t.settings.theme}>
            {#each themes as theme (theme)}
              <button
                role="radio"
                aria-checked={prefs.theme === theme}
                class:on={prefs.theme === theme}
                onclick={() => save({ theme })}
              >
                {t.settings.themes[theme]}
              </button>
            {/each}
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{t.settings.mascotTitle}</h2>
        <p class="hint">{t.settings.mascotHint}</p>
        <div class="bodies" role="radiogroup" aria-label={t.settings.mascotTitle}>
          {#each MASCOT_STYLES as id (id)}
            <button
              class="body-card"
              role="radio"
              aria-checked={prefs.mascotStyle === id}
              class:on={prefs.mascotStyle === id}
              onclick={() => setMascot({ mascotStyle: id })}
              onpointerenter={() => (hoveredBody = id)}
              onpointerleave={() => (hoveredBody = null)}
            >
              <Pip
                size={104}
                mood={prefs.mascotStyle === id ? "happy" : "idle"}
                style={id}
                fps={24}
                paused={prefs.mascotStyle !== id && hoveredBody !== id}
                label={t.settings.mascotStyles[id]?.name ?? id}
              />
              <span class="body-name">{t.settings.mascotStyles[id]?.name}</span>
              <span class="body-look">{t.settings.mascotStyles[id]?.look}</span>
            </button>
          {/each}
        </div>
        <div class="row">
          <span>{t.settings.mascotColor}</span>
          <div class="swatches" role="radiogroup" aria-label={t.settings.mascotColor}>
            {#each MASCOT_COLORS as swatch (swatch.id)}
              <button
                class="swatch"
                role="radio"
                aria-checked={prefs.mascotColor === swatch.color}
                aria-label={t.settings.mascotColors[swatch.id]}
                title={t.settings.mascotColors[swatch.id]}
                class:on={prefs.mascotColor === swatch.color}
                style:--swatch={swatch.color}
                onclick={() => setMascot({ mascotColor: swatch.color })}
              ></button>
            {/each}
            <label
              class="swatch custom"
              class:on={!MASCOT_COLORS.some((s) => s.color === prefs?.mascotColor)}
              title={t.settings.mascotCustomColor}
              style:--swatch={prefs.mascotColor}
            >
              <input
                type="color"
                value={prefs.mascotColor}
                aria-label={t.settings.mascotCustomColor}
                oninput={(e) => mascot.apply({ ...prefs!, mascotColor: e.currentTarget.value })}
                onchange={(e) => setMascot({ mascotColor: e.currentTarget.value })}
              />
            </label>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{t.settings.expressionsTitle}</h2>
        <p class="hint">{t.settings.expressionsHint}</p>
        <div class="expr-stage" aria-live="polite">
          <Pip size={112} mood={expression} fps={30} label={t.pipLabel} />
          <div>
            <span class="expr-name">{t.settings.expressions[expression]?.name}</span>
            <span class="expr-look">{t.settings.expressions[expression]?.look}</span>
          </div>
        </div>
        <div class="expr-grid" role="radiogroup" aria-label={t.settings.expressionsTitle}>
          {#each GALLERY as id (id)}
            <button
              class="expr"
              role="radio"
              aria-checked={expression === id}
              class:on={expression === id}
              onclick={() => (expression = id)}
              onpointerenter={() => (hovered = id)}
              onpointerleave={() => (hovered = null)}
            >
              <!-- Thumbnails hold still until hovered, so 20 canvases don't all animate. -->
              <Pip
                size={60}
                mood={id}
                fps={24}
                paused={hovered !== id}
                label={t.settings.expressions[id]?.name ?? id}
              />
              <span>{t.settings.expressions[id]?.name}</span>
            </button>
          {/each}
        </div>
      </section>
      <DataPanel />
    {:else if page === "notch"}
      <section class="card">
        <h2>{t.settings.appearance}</h2>
        <div class="stage" role="img" aria-label={t.settings.preview}>
          <div class="screen">
            <div class="wallpaper"></div>
            <div
              class="mini-notch"
              data-position={prefs.islandPosition}
              style:border-radius="0 0 {radius}px {radius}px"
            >
              <Pip size={18} label={t.pipLabel} />
              <span>{t.appName}</span>
              <span class="mini-time">{clock}</span>
            </div>
            <div class="taskbar" aria-hidden="true">
              <span class="tb-icons">
                <span class="tb win"></span>
                <span class="tb search"></span>
                <span class="tb folder"></span>
                <span class="tb browser"></span>
              </span>
              <span class="tb-clock">{clock}</span>
            </div>
          </div>
        </div>

        <div class="field">
          <span class="label">{t.settings.position}</span>
          <div class="choices" role="radiogroup" aria-label={t.settings.position}>
            {#each positions as position (position)}
              <button
                role="radio"
                aria-checked={prefs.islandPosition === position}
                class:on={prefs.islandPosition === position}
                onclick={() => save({ islandPosition: position })}
              >
                {t.settings.positions[position]}
              </button>
            {/each}
          </div>
        </div>
        <label class="field">
          <span class="label">{t.settings.monitor}</span>
          <select
            value={prefs.monitor ?? ""}
            onchange={(e) => save({ monitor: e.currentTarget.value || null })}
          >
            <option value="">{t.settings.primaryMonitor}</option>
            {#each monitors as monitor (monitor.name)}
              <option value={monitor.name}>
                {t.settings.monitorLabel(
                  monitor.name,
                  monitor.width,
                  monitor.height,
                  monitor.primary,
                )}
              </option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="label">{t.settings.pillRadius}</span>
          <input
            class="slider"
            type="range"
            min="0"
            max="24"
            value={prefs.pillRadius}
            style:--fill="{(radius / 24) * 100}%"
            oninput={(e) => (radiusDraft = Number(e.currentTarget.value))}
            onchange={(e) => {
              radiusDraft = null;
              void save({ pillRadius: Number(e.currentTarget.value) });
            }}
          />
          <span class="value">{t.settings.radiusValue(radius)}</span>
        </label>
      </section>

      <section class="card">
        <h2>{t.settings.tilesTitle}</h2>
        <p class="hint">{t.settings.tilesHint}</p>
        <div class="tiles">
          {#each notchTiles as id (id)}
            {@const on = prefs.notchModules.includes(id)}
            {@const last = on && prefs.notchModules.length === 1}
            <label
              class="tile"
              class:on
              style:--accent={accentFor(id)}
              title={last ? t.settings.keepOne : undefined}
            >
              <input type="checkbox" checked={on} disabled={last} onchange={() => toggleTile(id)} />
              <span class="tile-icon"><Icon name={id} size={32} /></span>
              <span class="tile-name">{t.modules[id]}</span>
            </label>
          {/each}
        </div>
      </section>

      <section class="card inline">
        <h2>{t.settings.shortcutKeys}</h2>
        {#if shortcutKeys.length > 0}
          <span class="keys">
            {#each shortcutKeys as key, i (key)}
              {#if i > 0}<span class="plus">+</span>{/if}
              <kbd>{key}</kbd>
            {/each}
          </span>
          <span class="hint">{t.settings.shortcutHint}</span>
        {:else}
          <span class="hint">{t.settings.shortcutNone}</span>
        {/if}
      </section>

      <section class="card inline">
        <h2>{t.settings.captureKeys}</h2>
        {#if info?.captureShortcut}
          <span class="keys">
            {#each formatShortcut(info.captureShortcut).split("+") as key, i (key)}
              {#if i > 0}<span class="plus">+</span>{/if}
              <kbd>{key}</kbd>
            {/each}
          </span>
          <span class="hint">{t.settings.captureHint}</span>
        {:else}
          <span class="hint">{t.settings.captureNone}</span>
        {/if}
      </section>
    {:else if page === "modules"}
      {#each visibleModules as id (id)}
        {@const module = modules.find((m) => m.id === id)}
        {@const activities = ACTIVITIES.filter((activity) => activity.module === id)}
        {#if module || id === "bridge"}
          <section class="card module" class:off={!module?.enabled} style:--accent={accentFor(id)}>
            <div class="module-head">
              {#if id === "productivity" || id === "monitor" || id === "wellbeing"}
                <span class="module-icon mascot"><ModuleMascot module={id} size={44} /></span>
              {:else}
                <span class="module-icon"><Icon name={id} size={22} /></span>
              {/if}
              <span class="module-text">
                <span class="module-name">
                  {t.modules[id] ?? id}
                  {#if module && module.status.state !== "stopped"}
                    <span class="badge" data-state={module.status.state}>
                      {t.settings.moduleStatus[module.status.state]}
                    </span>
                  {/if}
                </span>
                <span class="module-about">{t.settings.moduleAbout[id] ?? ""}</span>
                {#if module?.status.state === "degraded"}
                  <span class="module-problem">{module.status.message}</span>
                {/if}
              </span>
              {#if module}
                <label class="switch-label">
                  <span>{module.enabled ? t.settings.moduleOn : t.settings.moduleOff}</span>
                  <input
                    type="checkbox"
                    role="switch"
                    aria-label={t.settings.moduleToggle(t.modules[id] ?? id)}
                    checked={module.enabled}
                    onchange={(e) => setModule(id, e.currentTarget.checked)}
                  />
                </label>
              {:else}
                <span class="badge soon">{t.comingInPhase(6)}</span>
              {/if}
            </div>

            {#if module && !module.enabled}
              <p class="off-note">{t.settings.moduleOffNote}</p>
            {:else if module}
              {#if id === "bridge"}
                <div class="sub"><BridgePanel /></div>
              {/if}
              {#if id === "productivity"}
                <!-- Optional: offered here only, never pushed. -->
                <div class="sub">
                  {#if singleModule === "productivity"}
                    <CalendarPanel />
                  {:else}
                    <button class="ghost" onclick={() => openModule("productivity")}>
                      {t.calendar.openHint} ›
                    </button>
                  {/if}
                </div>
              {/if}
              {#if id === "monitor"}
                <div class="sub">
                  {#if singleModule === "monitor"}
                    <label class="row">
                      <span>
                        {t.monitor.devTools}
                        <small>{t.monitor.devToolsHint}</small>
                      </span>
                      <input
                        type="checkbox"
                        role="switch"
                        checked={prefs.devTools}
                        onchange={(e) => save({ devTools: e.currentTarget.checked })}
                      />
                    </label>
                    <SystemPanel />
                  {:else}
                    <button class="ghost" onclick={() => openModule("monitor")}>
                      {t.monitor.editAlerts} ›
                    </button>
                  {/if}
                </div>
              {/if}
              {#if id === "wellbeing"}
                <!-- The full panel on Wellbeing's own page; a way there from the overview. -->
                <div class="sub">
                  {#if singleModule === "wellbeing"}
                    <WellbeingPanel />
                  {:else}
                    <button class="ghost" onclick={() => openModule("wellbeing")}>
                      {t.wellbeing.openPageHint} ›
                    </button>
                  {/if}
                </div>
              {/if}
              {#if id === "weather"}
                <div class="sub">
                  <p class="hint">{t.settings.weatherHint}</p>
                  {#if prefs.weatherPlace && !changingPlace}
                    <div class="row">
                      <span>
                        📍 {prefs.weatherPlace.name}
                        {#if prefs.weatherPlace.detail}<small>{prefs.weatherPlace.detail}</small
                          >{/if}
                      </span>
                      <span class="actions">
                        <button class="ghost" onclick={() => (changingPlace = true)}>
                          {t.settings.weatherChange}
                        </button>
                        <button class="ghost" onclick={() => setWeather({ weatherPlace: null })}>
                          {t.settings.weatherRemove}
                        </button>
                      </span>
                    </div>
                  {:else}
                    <div class="place-search">
                      <input
                        class="text"
                        type="search"
                        bind:value={weatherQuery}
                        oninput={() => searchPlaces(weatherQuery)}
                        placeholder={t.settings.weatherSearch}
                        aria-label={t.settings.weatherSearch}
                      />
                      {#if weatherSearching}
                        <p class="muted">{t.settings.weatherSearching}</p>
                      {:else if weatherQuery.trim().length >= 2 && weatherResults.length === 0}
                        <p class="muted">{t.settings.weatherNoResults}</p>
                      {/if}
                      {#if weatherResults.length > 0}
                        <ul class="places">
                          {#each weatherResults as place (place.latitude + "," + place.longitude)}
                            <li>
                              <button onclick={() => choosePlace(place)}>
                                <span>{place.name}</span>
                                <small>{place.detail}</small>
                              </button>
                            </li>
                          {/each}
                        </ul>
                      {/if}
                      {#if !prefs.weatherPlace}<p class="muted">{t.settings.weatherOff}</p>{/if}
                    </div>
                  {/if}
                  <div class="row">
                    <span>{t.settings.unit}</span>
                    <div class="segmented" role="radiogroup" aria-label={t.settings.unit}>
                      {#each units as unit (unit)}
                        <button
                          role="radio"
                          aria-checked={prefs.temperatureUnit === unit}
                          class:on={prefs.temperatureUnit === unit}
                          onclick={() => setWeather({ temperatureUnit: unit })}
                        >
                          {t.settings.units[unit]}
                        </button>
                      {/each}
                    </div>
                  </div>
                </div>
              {/if}
              {#if activities.length > 0}
                <div class="sub">
                  <span class="eyebrow">{t.settings.inNotch}</span>
                  <div class="checks">
                    {#each activities as activity (activity.key)}
                      <label class="check">
                        <input
                          type="checkbox"
                          checked={prefs.notchActivities[activity.key] !== false}
                          onchange={(e) => setActivity(activity.key, e.currentTarget.checked)}
                        />
                        <span>{t.settings.activityLabels[activity.key]}</span>
                      </label>
                    {/each}
                  </div>
                </div>
              {/if}
            {/if}
          </section>
        {/if}
      {/each}
    {:else}
      <section class="card about">
        <Pip size={72} mood="happy" label={t.pipLabel} />
        <div>
          <h2>{info ? `${info.name} · ${t.settings.version(info.version)}` : t.appName}</h2>
          <p class="hint">{t.settings.privacy}</p>
          <p class="hint">{t.settings.madeWith}</p>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  main > header.hidden {
    display: none;
  }

  :global(html[data-window="settings"] body) {
    background:
      radial-gradient(1200px 600px at 100% 0%, var(--brand-soft), transparent 60%), var(--bg);
  }

  .layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    height: 100vh;
  }

  /* ---- Sidebar ---- */
  nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-6) var(--space-3) var(--space-5);
    border-right: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }

  .brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
  }

  .wordmark {
    font: 800 24px / 30px var(--font-ui);
    letter-spacing: -0.02em;
  }

  nav ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  nav button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: 10px var(--space-3);
    font: 500 14px / 20px var(--font-ui);
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  nav button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  nav button.active {
    color: var(--brand);
    background: var(--brand-soft);
    font-weight: 600;
  }

  /* Modules stays highlighted (softly) while one of its sub-pages is open. */
  nav button.parent {
    color: var(--text);
    font-weight: 600;
  }

  .chevron {
    display: grid;
    margin-left: auto;
    color: var(--text-muted);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .chevron.open {
    transform: rotate(180deg);
  }

  /* Collapses with a 0fr → 1fr grid row, so no height needs measuring. */
  .subnav {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows var(--dur-med, 200ms) var(--ease-out);
  }

  .subnav.open {
    grid-template-rows: 1fr;
  }

  .subnav > ul {
    overflow: hidden;
    gap: 1px;
    margin-left: 21px;
    padding-left: var(--space-2);
    border-left: 1px solid var(--border);
  }

  .subnav.open > ul {
    padding-block: var(--space-1);
  }

  nav .subitem {
    gap: var(--space-2);
    padding: 6px var(--space-2);
    font: 500 13px / 18px var(--font-ui);
  }

  nav .subitem.active {
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .sub-icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .subitem.off .sub-icon {
    filter: grayscale(1);
    opacity: 0.6;
  }

  .sub-name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sub-tag {
    margin-left: auto;
    font: var(--text-caption);
    color: var(--text-muted);
  }

  .crumb {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    margin-bottom: var(--space-1);
    font: 600 12px / 16px var(--font-ui);
    color: var(--brand);
  }

  .crumb:hover {
    text-decoration: underline;
  }

  .tagline {
    margin: auto 0 0;
    text-align: center;
    font: var(--text-caption);
    color: var(--text-muted);
  }

  /* ---- Main ---- */
  main {
    overflow-y: auto;
    padding: var(--space-8);
  }

  /* A readable column; wide rows made switches drift far from their labels. */
  main > * {
    max-width: 640px;
    margin-inline: auto;
  }

  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-6);
  }

  h1 {
    margin: 0;
    font: 800 26px / 32px var(--font-ui);
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: var(--space-1) 0 0;
    font: 400 14px / 20px var(--font-ui);
    color: var(--text-muted);
  }

  .autosave {
    flex: none;
    padding: 3px var(--space-3);
    font: var(--text-caption);
    color: var(--text-muted);
    white-space: nowrap;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .card {
    margin-bottom: var(--space-4);
    padding: var(--space-5) var(--space-6);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 1px 2px rgba(20, 20, 40, 0.04);
  }

  h2 {
    margin: 0 0 var(--space-1);
    font: 700 16px / 22px var(--font-ui);
  }

  h2:only-child,
  .card > h2:has(+ .stage) {
    margin-bottom: var(--space-4);
  }

  .hint,
  small,
  .muted {
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .hint {
    margin: 0 0 var(--space-4);
  }

  .muted {
    margin: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: 52px;
    padding: var(--space-2) 0;
    border-top: 1px solid var(--border);
  }

  .card > .row:first-child,
  .sub > .hint + .row {
    border-top: none;
  }

  .row > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font: 500 14px / 20px var(--font-ui);
  }

  .row small {
    font: 400 12px / 16px var(--font-ui);
  }

  /* ---- Your Pip: body + color (General) ---- */
  .bodies {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
    margin-bottom: var(--space-2);
  }

  .body-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: var(--space-3) var(--space-2) var(--space-4);
    text-align: center;
    background:
      radial-gradient(90% 70% at 50% 35%, var(--brand-soft), transparent 75%), var(--surface-2);
    border: 1.5px solid transparent;
    border-radius: var(--radius-md);
    transition:
      border-color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  .body-card:hover {
    transform: translateY(-2px);
  }

  .body-card.on {
    border-color: var(--brand);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--brand) 18%, transparent);
  }

  .body-name {
    margin-top: var(--space-1);
    font: 700 14px / 20px var(--font-ui);
  }

  .body-card.on .body-name {
    color: var(--brand);
  }

  .body-look {
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    justify-content: flex-end;
  }

  .swatch {
    position: relative;
    width: 28px;
    height: 28px;
    padding: 0;
    background: var(--swatch);
    border: 1px solid color-mix(in srgb, var(--text) 18%, transparent);
    border-radius: 50%;
    cursor: pointer;
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .swatch:hover {
    transform: scale(1.1);
  }

  .swatch.on {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 4px var(--brand);
  }

  /* The custom swatch is a rainbow ring around the current color; the native picker is hidden. */
  .swatch.custom {
    background:
      radial-gradient(circle, var(--swatch) 0 45%, transparent 47%),
      conic-gradient(#ff6b6b, #ffd86b, #9be3b4, #9cc8ff, #c6b2ff, #ff6b6b);
  }

  .swatch.custom input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  @media (max-width: 820px) {
    .bodies {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  /* ---- Expression gallery (General) ---- */
  .expr-stage {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    margin-bottom: var(--space-4);
    padding: var(--space-3) var(--space-5);
    background:
      radial-gradient(120% 140% at 0% 0%, var(--brand-soft), transparent 70%), var(--surface-2);
    border-radius: var(--radius-md);
  }

  .expr-stage > div {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .expr-name {
    font: 800 20px / 26px var(--font-ui);
    letter-spacing: -0.01em;
  }

  .expr-look {
    font: 400 14px / 20px var(--font-ui);
    color: var(--text-muted);
  }

  .expr-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--space-2);
  }

  .expr {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-1) var(--space-2);
    font: 500 12px / 16px var(--font-ui);
    color: var(--text-muted);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .expr:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .expr.on {
    color: var(--brand);
    font-weight: 600;
    background: var(--brand-soft);
    border-color: color-mix(in srgb, var(--brand) 35%, transparent);
  }

  /* ---- Modules page ---- */
  .module {
    padding: var(--space-4) var(--space-5);
  }

  .module-head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .module-icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-md);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    transition: filter var(--dur-fast) var(--ease-out);
  }

  .module-icon.mascot {
    background: none;
  }

  .module.off .module-icon {
    filter: grayscale(1);
    opacity: 0.6;
  }

  .module-text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .module-name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font: 700 15px / 20px var(--font-ui);
  }

  .module-about {
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .module-problem {
    font: 500 12px / 16px var(--font-ui);
    color: var(--danger);
  }

  .badge {
    padding: 1px var(--space-2);
    font: 600 11px / 16px var(--font-ui);
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, transparent);
    border-radius: var(--radius-pill);
  }

  .badge[data-state="degraded"] {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }

  .badge.soon {
    flex: none;
    color: var(--text-muted);
    background: var(--surface-2);
  }

  .switch-label {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    font: 500 13px / 18px var(--font-ui);
    color: var(--text-muted);
    cursor: pointer;
  }

  /* Indented under the module name (icon 44 px + gap 16 px). */
  .off-note,
  .sub {
    margin-left: 60px;
  }

  .off-note {
    margin-top: var(--space-2);
    margin-bottom: 0;
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .sub {
    margin-top: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }

  .eyebrow {
    display: block;
    margin-bottom: var(--space-2);
    font: 600 11px / 14px var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .checks {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0 var(--space-4);
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 6px 0;
    font: 500 13px / 18px var(--font-ui);
    cursor: pointer;
  }

  .check input {
    flex: none;
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--brand);
    cursor: pointer;
  }

  /* ---- Notch page ---- */
  .stage {
    padding: var(--space-5);
    margin-bottom: var(--space-4);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .screen {
    position: relative;
    aspect-ratio: 16 / 7;
    overflow: hidden;
    border: 8px solid #17171d;
    border-radius: 14px;
    background: #c9c3ff;
  }

  .wallpaper {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(120% 90% at 0% 120%, rgba(124, 92, 255, 0.35), transparent 55%),
      radial-gradient(110% 80% at 100% 110%, rgba(124, 92, 255, 0.45), transparent 60%),
      linear-gradient(180deg, #d9d4ff, #b9b0ff);
  }

  .mini-notch {
    position: absolute;
    top: 0;
    /* Never shrink-to-fit against the screen edge (top right wrapped the time). */
    width: max-content;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px 7px 12px;
    font: 600 13px / 16px var(--font-ui);
    color: #f5f5f7;
    background: #0b0b0f;
    transition:
      left var(--dur-slow) var(--ease-out),
      transform var(--dur-slow) var(--ease-out),
      border-radius var(--dur-fast) var(--ease-out);
  }

  .mini-notch[data-position="left"] {
    left: 16px;
  }

  .mini-notch[data-position="center"] {
    left: 50%;
    transform: translateX(-50%);
  }

  .mini-notch[data-position="right"] {
    left: calc(100% - 16px);
    transform: translateX(-100%);
  }

  .mini-time {
    margin-left: 18px;
    font: var(--text-mono);
  }

  .taskbar {
    position: absolute;
    inset: auto 0 0;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 30px;
    background: rgba(245, 245, 250, 0.92);
  }

  .tb-icons {
    display: flex;
    gap: 12px;
  }

  .tb {
    width: 14px;
    height: 14px;
    border-radius: 3px;
  }

  .tb.win {
    background: linear-gradient(135deg, #3ea0ff, #1677d9);
  }

  .tb.search {
    border: 2px solid #33333d;
    border-radius: 50%;
    width: 11px;
    height: 11px;
  }

  .tb.folder {
    background: #f4c542;
  }

  .tb.browser {
    border-radius: 50%;
    background: conic-gradient(#36c, #2cb5a0, #36c);
  }

  .tb-clock {
    position: absolute;
    right: 12px;
    font: 500 10px / 12px var(--font-ui);
    color: #33333d;
  }

  .field {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }

  .label {
    font: 600 14px / 20px var(--font-ui);
  }

  .choices {
    grid-column: 2 / 4;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-2);
  }

  .choices button {
    height: 38px;
    font: 500 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .choices button:hover {
    border-color: color-mix(in srgb, var(--brand) 50%, var(--border));
  }

  .choices button.on {
    color: #fff;
    background: var(--brand);
    border-color: var(--brand);
  }

  select {
    grid-column: 2 / 4;
    height: 40px;
    padding: 0 var(--space-3);
    font: 500 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .slider {
    appearance: none;
    height: 6px;
    border-radius: var(--radius-pill);
    background: linear-gradient(90deg, var(--brand) var(--fill), var(--surface-2) var(--fill));
    cursor: pointer;
  }

  .slider::-webkit-slider-thumb {
    appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--brand);
    border: 3px solid var(--surface);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
  }

  .value {
    min-width: 48px;
    font: var(--text-body);
    color: var(--text-muted);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
  }

  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-2) var(--space-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }

  .tile:not(.on) {
    opacity: 0.7;
  }

  .tile.on {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
  }

  .tile:has(input:disabled) {
    cursor: not-allowed;
  }

  .tile input {
    position: absolute;
    top: var(--space-3);
    right: var(--space-3);
    width: 20px;
    height: 20px;
    margin: 0;
    accent-color: var(--brand);
    cursor: inherit;
  }

  .tile-icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .tile-name {
    font: 600 15px / 20px var(--font-ui);
  }

  .card.inline {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }

  .card.inline h2,
  .card.inline .hint {
    margin: 0;
  }

  .keys {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  kbd {
    min-width: 44px;
    padding: var(--space-2) var(--space-3);
    text-align: center;
    font: 500 14px / 18px var(--font-ui);
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .plus {
    color: var(--text-muted);
  }

  .segmented {
    display: inline-flex;
    padding: 2px;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }

  .segmented button {
    padding: var(--space-1) var(--space-3);
    font: var(--text-pill);
    color: var(--text-muted);
    border-radius: calc(var(--radius-sm) - 2px);
  }

  .segmented button.on {
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }

  .about {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }

  .about .hint {
    margin: var(--space-1) 0 0;
  }

  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 42px;
    height: 24px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--surface-2);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--brand);
    border-color: var(--brand);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    transform: translateX(18px);
  }

  .error {
    padding: var(--space-2) var(--space-3);
    color: var(--danger);
    background: var(--surface);
    border: 1px solid var(--danger);
    border-radius: var(--radius-sm);
  }

  @media (max-width: 820px) {
    .tiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  /* ---- Weather picker ---- */
  .actions {
    flex-direction: row !important;
    gap: var(--space-2) !important;
  }

  .ghost {
    padding: var(--space-1) var(--space-3);
    font: var(--text-pill);
    color: var(--brand);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .ghost:hover {
    background: var(--brand-soft);
  }

  .place-search {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }

  .text {
    height: 40px;
    padding: 0 var(--space-3);
    font: 500 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .text:focus-visible {
    outline: none;
    border-color: var(--brand);
  }

  .places {
    margin: 0;
    padding: var(--space-1);
    list-style: none;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .places button {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    text-align: left;
    border-radius: calc(var(--radius-sm) - 2px);
  }

  .places button:hover,
  .places button:focus-visible {
    background: var(--brand-soft);
  }

  .places span {
    font: 600 14px / 20px var(--font-ui);
  }
</style>
