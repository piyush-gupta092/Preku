<script lang="ts">
  // Settings → General: interruptions (quiet mode, usability U4) and "Your data" (U5: pause
  // clipboard capture, how long things are kept, export, delete). Everything stays on this
  // PC; an export never contains saved sign-ins or links.
  import { onMount } from "svelte";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { pauseUntil, quiet } from "../../lib/stores/quiet.svelte";
  import type { PrivacySettings } from "../../lib/types/generated/PrivacySettings";

  let privacy = $state<PrivacySettings | null>(null);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);
  let confirming = $state(false);
  let now = $state(Date.now());

  const until = $derived(quiet.pausedUntil(now));

  onMount(() => {
    let off: (() => void) | undefined;
    void quiet.connect().then((unlisten) => (off = unlisten));
    void ipc.privacyGet().then((result) => {
      if (result.ok) privacy = result.value;
    });
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      clearInterval(timer);
      off?.();
    };
  });

  async function setPrivacy(change: Partial<PrivacySettings>) {
    if (!privacy) return;
    const result = await ipc.privacySet({ ...privacy, ...change });
    if (result.ok) {
      privacy = result.value;
      error = null;
    } else {
      error = result.error.message;
    }
  }

  async function exportData() {
    message = null;
    const result = await ipc.privacyExport();
    if (!result.ok) error = result.error.message;
    else if (result.value) message = t.data.exported(result.value);
  }

  async function deleteAll() {
    confirming = false;
    const result = await ipc.privacyDeleteAll();
    if (result.ok) {
      message = t.data.deleted;
      error = null;
    } else {
      error = result.error.message;
    }
  }

  function clock(ms: number): string {
    return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  }
</script>

<section class="card">
  <h2>{t.quiet.title}</h2>
  <p class="hint">{t.quiet.intro}</p>
  <label class="row">
    <span>
      {t.quiet.holdInFocus}
      <small>{t.quiet.holdInFocusHint}</small>
    </span>
    <input
      type="checkbox"
      role="switch"
      checked={quiet.state.holdInFocus}
      onchange={(e) => quiet.holdInFocus(e.currentTarget.checked)}
    />
  </label>
  <label class="row">
    <span>
      {t.quiet.holdFullscreen}
      <small>{t.quiet.holdFullscreenHint}</small>
    </span>
    <input
      type="checkbox"
      role="switch"
      checked={quiet.state.holdFullscreenApps}
      onchange={(e) => quiet.holdFullscreenApps(e.currentTarget.checked)}
    />
  </label>
  <div class="row">
    <span>
      {t.quiet.pause}
      <small>{until !== null ? t.quiet.pausedUntil(clock(until)) : t.quiet.note}</small>
    </span>
    <span class="actions">
      {#if until !== null}
        <button class="primary" onclick={() => quiet.pause(null)}>{t.quiet.resume}</button>
      {:else}
        <button class="ghost" onclick={() => quiet.pause(pauseUntil(30, Date.now()))}>
          {t.quiet.for30}
        </button>
        <button class="ghost" onclick={() => quiet.pause(pauseUntil(60, Date.now()))}>
          {t.quiet.for60}
        </button>
        <button class="ghost" onclick={() => quiet.pause(pauseUntil("tomorrow", Date.now()))}>
          {t.quiet.tomorrow}
        </button>
      {/if}
    </span>
  </div>
</section>

{#if privacy}
  <section class="card">
    <h2>{t.data.title}</h2>
    <p class="hint">{t.data.intro}</p>
    <label class="row">
      <span>
        {t.data.pauseClipboard}
        <small>{t.data.pauseClipboardHint}</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={privacy.clipboardPaused}
        onchange={(e) => setPrivacy({ clipboardPaused: e.currentTarget.checked })}
      />
    </label>
    <label class="row">
      <span>
        {t.data.keepClipboard}
        <small>{t.data.keepClipboardHint}</small>
      </span>
      <select
        value={privacy.clipboardKeepDays}
        onchange={(e) => setPrivacy({ clipboardKeepDays: Number(e.currentTarget.value) })}
      >
        <option value={0}>{t.data.last50}</option>
        {#each [1, 7, 30, 90] as days (days)}
          <option value={days}>{t.data.days(days)}</option>
        {/each}
      </select>
    </label>
    <label class="row">
      <span>
        {t.data.keepUsage}
        <small>{t.data.keepUsageHint}</small>
      </span>
      <select
        value={privacy.usageKeepDays}
        onchange={(e) => setPrivacy({ usageKeepDays: Number(e.currentTarget.value) })}
      >
        <option value={0}>{t.data.forever}</option>
        {#each [30, 90, 365] as days (days)}
          <option value={days}>{t.data.days(days)}</option>
        {/each}
      </select>
    </label>
    <div class="row">
      <span>
        {t.data.export}
        <small>{t.data.exportHint}</small>
      </span>
      <button class="ghost" onclick={exportData}>{t.data.exportButton}</button>
    </div>
    <div class="row">
      <span>
        {t.data.delete}
        <small>{t.data.deleteHint}</small>
      </span>
      {#if confirming}
        <span class="actions">
          <button class="danger" onclick={deleteAll}>{t.data.deleteConfirm}</button>
          <button class="ghost" onclick={() => (confirming = false)}>{t.data.keep}</button>
        </span>
      {:else}
        <button class="ghost danger-text" onclick={() => (confirming = true)}>
          {t.data.deleteButton}
        </button>
      {/if}
    </div>
    {#if confirming}
      <p class="warn" role="alert">{t.data.deleteWarning}</p>
    {/if}
    {#if message}<p class="hint" role="status">{message}</p>{/if}
    {#if error}<p class="warn" role="alert">{error}</p>{/if}
  </section>
{/if}

<style>
  /* Looks like the other General cards (their styles are scoped to Settings.svelte). */
  .card {
    margin-bottom: var(--space-4);
    padding: 20px 24px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  h2 {
    margin: 0 0 var(--space-1);
    font: 700 16px / 22px var(--font-ui);
  }

  .hint {
    margin: 0 0 var(--space-2);
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
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

  .row > span:first-child {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font: 500 14px / 20px var(--font-ui);
  }

  small {
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .actions {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  select {
    flex: none;
    padding: 6px var(--space-2);
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  button {
    flex: none;
    padding: 6px var(--space-3);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .ghost {
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .primary {
    color: #fff;
    background: var(--brand);
  }

  .danger {
    color: #fff;
    background: var(--danger);
  }

  .danger-text {
    color: var(--danger);
  }

  .warn {
    margin: var(--space-2) 0 0;
    font: 500 13px / 18px var(--font-ui);
    color: var(--danger);
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
</style>
