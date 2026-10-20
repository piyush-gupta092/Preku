<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { formatDuration } from "../../../lib/format";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { wellbeing } from "../../../lib/stores/wellbeing.svelte";
  import { companion } from "../../../lib/stores/companion.svelte";
  import { MOODS } from "../../../lib/companion";
  import Ring from "../tabs/Ring.svelte";

  let { onOpenTab, onOpenPage }: { onOpenTab: (tab: string) => void; onOpenPage: () => void } =
    $props();

  const state = $derived(wellbeing.state);
  const habitsDone = $derived(state?.habits.filter((h) => h.doneToday).length ?? 0);
  const todaysMood = $derived(MOODS.find((m) => m.id === state?.moodToday));
</script>

{#if state}
  <div class="today">
    <section class="water">
      <Ring size={120} value={state.waterMl / state.settings.waterGoalMl} stroke={8}>
        <div class="readout">
          <span class="ml">{state.waterMl}</span>
          <span class="pk-muted">{t.wellbeing.goal(state.settings.waterGoalMl)}</span>
        </div>
      </Ring>
      <div class="water-actions">
        <button class="pk-button" onclick={() => wellbeing.run(ipc.waterAdd())}>
          💧 {t.wellbeing.addGlass(state.settings.glassMl)}
        </button>
        <button
          class="pk-icon-button"
          onclick={() => wellbeing.run(ipc.waterUndo())}
          disabled={state.waterMl === 0}
          aria-label={t.wellbeing.undo}
          title={t.wellbeing.undo}
        >
          <Icon name="restart" size={14} />
        </button>
      </div>
    </section>

    <section class="side">
      {#if state.dnd}
        <p class="badge dnd"><Icon name="bell" size={14} />{t.wellbeing.dnd}</p>
      {/if}
      <div class="card">
        <Icon name="wellbeing" size={18} />
        <span>
          {state.nextBreakS !== null
            ? t.wellbeing.nextBreak(formatDuration(state.nextBreakS))
            : t.wellbeing.breaksOff}
          {#if state.breaksToday > 0}
            <span class="pk-muted"> · {t.companion.breaksToday(state.breaksToday)}</span>
          {/if}
        </span>
      </div>
      <button class="card link" onclick={() => onOpenTab("habits")}>
        <Icon name="check" size={18} />
        <span>
          {state.habits.length > 0
            ? t.wellbeing.habitsDone(habitsDone, state.habits.length)
            : t.wellbeing.noHabits}
        </span>
      </button>
      <button class="card link" onclick={onOpenPage}>
        <Icon name="monitor" size={18} />
        <span>{t.wellbeing.screenTotal} · {formatDuration(state.screenTotalS)}</span>
      </button>
      <div class="companion-actions">
        <!-- A breather on request (mental reset), and the optional mood check-in. -->
        <button class="pk-button" onclick={() => companion.take("reset")}>
          🫧 {t.companion.breather}
        </button>
        {#if state.settings.moodCheckInEnabled}
          {#if todaysMood}
            <button class="pk-button" onclick={() => companion.take("moodCheckIn")}>
              {todaysMood.emoji}
              {t.companion.moodToday(t.companion.moods[todaysMood.id] ?? "")}
            </button>
          {:else}
            <button class="pk-button" onclick={() => companion.take("moodCheckIn")}>
              💬 {t.companion.checkInNow}
            </button>
          {/if}
        {/if}
        <button class="open" onclick={onOpenPage}>{t.wellbeing.openPage} ›</button>
      </div>
    </section>
  </div>
{/if}

<style>
  .today {
    display: flex;
    gap: var(--space-5);
    align-items: center;
    height: 100%;
    padding: 0 var(--space-2);
  }

  .water {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
  }

  .readout {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .ml {
    font: var(--text-display);
    font-family: var(--font-mono);
  }

  .water-actions {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    text-align: left;
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
  }

  .card :global(svg) {
    color: var(--accent);
  }

  .link:hover {
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }

  /* A quiet way to the full page; the notch stays a glance. */
  .open {
    padding: var(--space-1) var(--space-2);
    font: var(--text-pill);
    color: var(--island-text-muted);
  }

  .open:hover {
    color: var(--island-text);
  }

  .companion-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .badge {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    padding: var(--space-1) var(--space-3);
    font: var(--text-caption);
    border-radius: var(--radius-pill);
  }

  .dnd {
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 14%, transparent);
  }
</style>
