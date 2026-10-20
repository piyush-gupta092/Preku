<script lang="ts">
  // Settings → Modules → Productivity → Calendar. Strictly optional (Rules.md §4): nothing is
  // connected, fetched or suggested until the user adds a calendar here; removing one deletes
  // its saved link and its events. Read-only.
  import { onMount } from "svelte";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { calendar } from "../../lib/stores/calendar.svelte";
  import type { AgendaEvent } from "../../lib/types/generated/AgendaEvent";

  let adding = $state<"link" | null>(null);
  let name = $state("");
  let link = $state("");
  let help = $state(false);
  let confirming = $state<string | null>(null);
  let now = $state(Date.now());

  const current = $derived(calendar.state);
  const sources = $derived(current?.settings.sources ?? []);

  onMount(() => {
    let off: (() => void) | undefined;
    void calendar.connect().then((unlisten) => (off = unlisten));
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      clearInterval(timer);
      off?.();
    };
  });

  async function addLink() {
    if (!link.trim()) return;
    const result = await calendar.run(ipc.calendarAddLink(name, link));
    if (result.ok) {
      name = "";
      link = "";
      adding = null;
    }
  }

  function time(ms: number): string {
    return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  }

  function span(event: AgendaEvent): string {
    if (event.allDay) return t.calendar.allDay;
    return event.end > event.start
      ? `${time(event.start)} – ${time(event.end)}`
      : time(event.start);
  }

  function status(id: string): string {
    const found = current?.statuses.find((s) => s.id === id);
    if (found?.error) return found.error;
    if (!found?.lastOk) return t.calendar.loading;
    const minutes = Math.max(0, Math.round((now - found.lastOk) / 60_000));
    return minutes < 1 ? t.calendar.updatedNow : t.calendar.updated(minutes);
  }

  function setOptions(change: { remindMin?: number; hideTitles?: boolean }) {
    const settings = current?.settings;
    if (!settings) return;
    void calendar.run(
      ipc.calendarSetOptions(
        change.remindMin ?? settings.remindMin,
        change.hideTitles ?? settings.hideTitles,
      ),
    );
  }
</script>

<div class="calendar">
  <h3>{t.calendar.title} <span class="optional">{t.calendar.optional}</span></h3>
  <p class="muted">{t.calendar.intro}</p>

  {#if sources.length > 0}
    <ul class="sources">
      {#each sources as source (source.id)}
        {@const failed = !!current?.statuses.find((s) => s.id === source.id)?.error}
        <li>
          <input
            type="checkbox"
            role="switch"
            aria-label={t.calendar.show(source.name)}
            checked={source.enabled}
            onchange={(e) =>
              calendar.run(ipc.calendarSetEnabled(source.id, e.currentTarget.checked))}
          />
          <span class="who">
            <span class="name">{source.name}</span>
            <small class:bad={failed && source.enabled} title={source.location}>
              {source.kind === "link" ? source.location : t.calendar.file} ·
              {source.enabled ? status(source.id) : t.calendar.off}
            </small>
          </span>
          {#if confirming === source.id}
            <small>{t.calendar.removeAsk}</small>
            <button
              class="danger"
              onclick={() => {
                confirming = null;
                void calendar.run(ipc.calendarRemove(source.id));
              }}>{t.calendar.remove}</button
            >
            <button class="ghost" onclick={() => (confirming = null)}>{t.calendar.keep}</button>
          {:else}
            <button class="ghost" onclick={() => (confirming = source.id)}>
              {t.calendar.remove}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if adding === "link"}
    <form
      class="add"
      onsubmit={(e) => {
        e.preventDefault();
        void addLink();
      }}
    >
      <input
        class="field"
        bind:value={link}
        type="url"
        placeholder={t.calendar.linkPlaceholder}
        aria-label={t.calendar.linkPlaceholder}
        autocomplete="off"
        spellcheck="false"
      />
      <input
        class="field short"
        bind:value={name}
        maxlength="40"
        placeholder={t.calendar.namePlaceholder}
        aria-label={t.calendar.namePlaceholder}
      />
      <button class="primary" type="submit" disabled={!link.trim()}>{t.calendar.add}</button>
      <button class="ghost" type="button" onclick={() => (adding = null)}>
        {t.calendar.cancel}
      </button>
    </form>
    <p class="muted">{t.calendar.linkPrivacy}</p>
    <button class="link" onclick={() => (help = !help)}>{t.calendar.whereLink}</button>
    {#if help}
      <ul class="help">
        <li><b>Google Calendar:</b> {t.calendar.helpGoogle}</li>
        <li><b>Outlook:</b> {t.calendar.helpOutlook}</li>
        <li><b>iCloud:</b> {t.calendar.helpIcloud}</li>
      </ul>
    {/if}
  {:else}
    <div class="actions">
      <button class="ghost" onclick={() => (adding = "link")}>{t.calendar.addLink}</button>
      <button class="ghost" onclick={() => calendar.run(ipc.calendarAddFile(""))}>
        {t.calendar.addFile}
      </button>
      {#if sources.some((s) => s.enabled)}
        <button class="link" onclick={() => calendar.run(ipc.calendarRefresh())}>
          {t.calendar.refresh}
        </button>
      {/if}
    </div>
  {/if}

  {#if calendar.error}
    <p class="error" role="alert">{calendar.error}</p>
  {/if}

  {#if current && sources.some((s) => s.enabled)}
    <h4>{t.calendar.today}</h4>
    <ul class="agenda">
      {#each current.today as event (event.id)}
        <li class:past={!event.allDay && event.end < now}>
          <span class="when">{span(event)}</span>
          <span class="what" title={event.title}>{event.title}</span>
          {#if sources.length > 1}<small>{event.sourceName}</small>{/if}
        </li>
      {:else}
        <li class="muted">{t.calendar.nothingToday}</li>
      {/each}
    </ul>

    <h4>{t.calendar.notch}</h4>
    <label class="row">
      <span class="grow">{t.calendar.remind}</span>
      <select
        class="field"
        value={current.settings.remindMin}
        onchange={(e) => setOptions({ remindMin: Number(e.currentTarget.value) })}
      >
        <option value={0}>{t.calendar.remindOff}</option>
        {#each [5, 10, 15, 30] as minutes (minutes)}
          <option value={minutes}>{t.calendar.remindBefore(minutes)}</option>
        {/each}
      </select>
    </label>
    <label class="row">
      <input
        type="checkbox"
        role="switch"
        checked={current.settings.hideTitles}
        onchange={(e) => setOptions({ hideTitles: e.currentTarget.checked })}
      />
      <span class="grow">{t.calendar.hideTitles}</span>
    </label>
    <p class="muted">{t.calendar.readOnly}</p>
  {/if}
</div>

<style>
  .calendar {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  h3 {
    margin: 0;
    font: 700 15px / 20px var(--font-ui);
  }

  .optional {
    margin-left: var(--space-1);
    padding: 1px 8px;
    font: 600 11px / 16px var(--font-ui);
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    vertical-align: 1px;
  }

  h4 {
    margin: var(--space-3) 0 0;
    font: 600 11px / 14px var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .muted {
    margin: 0;
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .sources li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .name {
    font: 600 14px / 20px var(--font-ui);
  }

  small {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  small.bad {
    color: var(--danger);
    white-space: normal;
  }

  .add,
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .field {
    flex: 1;
    min-width: 0;
    padding: 6px var(--space-2);
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .field.short {
    flex: 0 1 150px;
  }

  select.field {
    flex: none;
  }

  .help {
    padding: var(--space-3);
    gap: var(--space-2);
    font: 400 13px / 18px var(--font-ui);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .agenda li {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    min-height: 28px;
    font: 400 14px / 20px var(--font-ui);
  }

  .agenda li.past {
    opacity: 0.5;
  }

  .when {
    flex: none;
    width: 140px;
    font: 500 12px / 18px var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }

  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 36px;
    font: 500 14px / 20px var(--font-ui);
  }

  .grow {
    flex: 1;
  }

  button {
    padding: 6px var(--space-3);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .primary {
    color: #fff;
    background: var(--mod-productivity);
  }

  .primary:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .ghost {
    flex: none;
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .danger {
    color: #fff;
    background: var(--danger);
  }

  .link {
    align-self: flex-start;
    padding: 0;
    color: var(--brand);
    font: 600 13px / 18px var(--font-ui);
  }

  .link:hover {
    text-decoration: underline;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font: 500 13px / 18px var(--font-ui);
  }

  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 36px;
    height: 20px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--surface);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--mod-productivity);
    border-color: var(--mod-productivity);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    background: #fff;
    transform: translateX(16px);
  }
</style>
