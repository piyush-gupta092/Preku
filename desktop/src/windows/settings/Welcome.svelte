<script lang="ts">
  // The first-run welcome (usability U3): short, skippable, one screen. It starts from how
  // things are set right now, so someone who already uses Preku keeps their setup unless they
  // change a switch here. Clipboard history and app-usage tracking are explained and can be
  // declined separately. Everything here can be changed later in Settings.
  import { onMount } from "svelte";
  import Pip from "../../pip/Pip.svelte";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { wellbeing } from "../../lib/stores/wellbeing.svelte";

  interface Props {
    /** Module id → on right now. */
    modules: Record<string, boolean>;
    shortcut: string | null;
    /** Finished or skipped: `modules` is what to switch (null = change nothing). */
    ondone: (modules: Record<string, boolean> | null) => void;
  }
  let { modules, shortcut, ondone }: Props = $props();

  let wanted = $state<Record<string, boolean>>({});
  let touched = $state(false);
  // Start from how things are set now (the module list arrives a moment after the window).
  $effect(() => {
    if (!touched) wanted = { ...modules };
  });
  let clipboard = $state(true);
  let usage = $state(true);
  let busy = $state(false);

  const helps = ["productivity", "wellbeing", "monitor"] as const;

  onMount(() => {
    let off: (() => void) | undefined;
    void ipc.privacyGet().then((result) => {
      if (result.ok) clipboard = !result.value.clipboardPaused;
    });
    void wellbeing.connect().then((unlisten) => {
      off = unlisten;
      usage = wellbeing.state?.settings.screenTimeEnabled ?? true;
    });
    return () => off?.();
  });

  async function start() {
    busy = true;
    const privacy = await ipc.privacyGet();
    if (privacy.ok) await ipc.privacySet({ ...privacy.value, clipboardPaused: !clipboard });
    const settings = wellbeing.state?.settings;
    if (settings && settings.screenTimeEnabled !== usage) {
      await wellbeing.run(ipc.wellbeingSettingsSet({ ...settings, screenTimeEnabled: usage }));
    }
    ondone(wanted);
  }
</script>

<div class="welcome" role="dialog" aria-labelledby="welcome-title">
  <header>
    <Pip size={88} mood="happy" label={t.pipLabel} />
    <div>
      <h1 id="welcome-title">{t.welcome.title}</h1>
      <p>{t.welcome.intro}</p>
      <div class="top-actions">
        <button class="primary" onclick={start} disabled={busy}>{t.welcome.start}</button>
        <button class="ghost" onclick={() => ondone(null)} disabled={busy}>{t.welcome.skip}</button>
      </div>
    </div>
  </header>

  <section>
    <h2>{t.welcome.helpTitle}</h2>
    {#each helps as id (id)}
      <label class="choice">
        <input
          type="checkbox"
          checked={wanted[id] ?? false}
          onchange={(e) => {
            touched = true;
            wanted = { ...wanted, [id]: e.currentTarget.checked };
          }}
        />
        <span>
          <b>{t.welcome.help[id]?.name}</b>
          <small>{t.welcome.help[id]?.about}</small>
        </span>
      </label>
    {/each}
  </section>

  <section>
    <h2>{t.welcome.captureTitle}</h2>
    <p class="muted">{t.welcome.captureIntro}</p>
    <label class="choice" class:off={!wanted.productivity}>
      <input type="checkbox" bind:checked={clipboard} disabled={!wanted.productivity} />
      <span>
        <b>{t.welcome.clipboard}</b>
        <small>{t.welcome.clipboardAbout}</small>
      </span>
    </label>
    <label class="choice" class:off={!wanted.wellbeing}>
      <input type="checkbox" bind:checked={usage} disabled={!wanted.wellbeing} />
      <span>
        <b>{t.welcome.usage}</b>
        <small>{t.welcome.usageAbout}</small>
      </span>
    </label>
  </section>

  <section>
    <h2>{t.welcome.openTitle}</h2>
    <p class="muted">
      {t.welcome.openHow}
      {#if shortcut}{t.welcome.openShortcut(shortcut)}{/if}
    </p>
  </section>

  <footer>
    <button class="primary" onclick={start} disabled={busy}>{t.welcome.start}</button>
    <button class="ghost" onclick={() => ondone(null)} disabled={busy}>{t.welcome.skip}</button>
    <small>{t.welcome.later}</small>
  </footer>
</div>

<style>
  .welcome {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 640px;
    margin: 0 auto;
    padding: var(--space-2) 0 var(--space-6);
  }

  .top-actions {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }

  h1 {
    margin: 0 0 var(--space-1);
    font: 800 26px / 32px var(--font-ui);
  }

  header p,
  .muted {
    margin: 0;
    font: 400 14px / 20px var(--font-ui);
    color: var(--text-muted);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 16px 24px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  h2 {
    margin: 0;
    font: 700 16px / 22px var(--font-ui);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    cursor: pointer;
  }

  .choice.off {
    opacity: 0.5;
    cursor: default;
  }

  .choice input {
    flex: none;
    width: 18px;
    height: 18px;
    margin-top: 2px;
    accent-color: var(--brand);
  }

  .choice span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font: 500 14px / 20px var(--font-ui);
  }

  small {
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  button {
    padding: 8px var(--space-5);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .primary {
    color: #fff;
    background: var(--brand);
  }

  .ghost {
    color: var(--brand);
    border: 1px solid var(--border);
  }

  button:disabled {
    opacity: 0.5;
  }
</style>
