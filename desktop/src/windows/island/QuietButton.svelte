<script lang="ts">
  // "Pause reminders" in the notch header (usability U4): one tap to hold routine prompts
  // for a while, with the end time always visible and one tap to resume. Urgent alerts (low
  // battery, disk almost full) and what the user asked for still come through.
  import Icon from "../../lib/Icon.svelte";
  import { t } from "../../lib/i18n";
  import { pauseUntil, quiet } from "../../lib/stores/quiet.svelte";

  let { now }: { now: number } = $props();
  let open = $state(false);

  const until = $derived(quiet.pausedUntil(now));
  /** Not paused by hand, but holding because a focus session runs. */
  const holding = $derived(until === null && quiet.state.focusing && quiet.state.holdInFocus);

  function clock(ms: number): string {
    return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  }

  function choose(choice: 30 | 60 | "tomorrow") {
    open = false;
    void quiet.pause(pauseUntil(choice, Date.now()));
  }
</script>

<div class="quiet">
  {#if until !== null}
    <!-- Compact: the header is small. The end time stays visible; one tap resumes. -->
    <button
      class="chip on"
      onclick={() => quiet.pause(null)}
      aria-label="{t.quiet.until(clock(until))}. {t.quiet.resume}"
      title="{t.quiet.until(clock(until))}. {t.quiet.resume}"
    >
      <span class="zz" aria-hidden="true">🔕</span>
      <span>{clock(until)}</span>
    </button>
  {:else}
    <button
      class="bell"
      class:holding
      onclick={() => (open = !open)}
      aria-expanded={open}
      aria-label={holding ? t.quiet.holding : t.quiet.pause}
      title={holding ? t.quiet.holding : t.quiet.pause}
    >
      <Icon name="bell" size={18} />
    </button>
    {#if open}
      <div class="menu" role="menu">
        <span class="title">{t.quiet.pause}</span>
        <button role="menuitem" onclick={() => choose(30)}>{t.quiet.for30}</button>
        <button role="menuitem" onclick={() => choose(60)}>{t.quiet.for60}</button>
        <button role="menuitem" onclick={() => choose("tomorrow")}>{t.quiet.tomorrow}</button>
        <span class="note">{t.quiet.note}</span>
      </div>
    {/if}
  {/if}
</div>

<svelte:window
  onkeydown={(e) => {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      open = false;
    }
  }}
/>

<style>
  .quiet {
    position: relative;
    flex: none;
    display: flex;
    align-items: center;
  }

  .bell {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .bell:hover {
    color: var(--island-text);
    background: var(--island-bg-raised);
  }

  /* Holding during focus: a quiet hint, no number to read. */
  .bell.holding {
    color: var(--mod-productivity);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 var(--space-2);
    font: var(--text-pill);
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .zz {
    font-size: 12px;
  }

  .menu {
    position: absolute;
    top: 36px;
    right: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    width: 210px;
    padding: var(--space-2);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-island);
  }

  .title,
  .note {
    padding: var(--space-1) var(--space-2);
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .menu button {
    padding: var(--space-2);
    text-align: left;
    font: var(--text-body);
    color: var(--island-text);
    border-radius: var(--radius-sm);
  }

  .menu button:hover {
    background: var(--island-bg);
  }
</style>
