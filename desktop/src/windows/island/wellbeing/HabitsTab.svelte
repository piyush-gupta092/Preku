<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { wellbeing } from "../../../lib/stores/wellbeing.svelte";

  // Today's check-ins only. Adding, editing and deleting habits live on the Wellbeing page.
  let { onOpenPage }: { onOpenPage: () => void } = $props();
</script>

<div class="habits">
  <ul class="pk-scroll">
    {#each wellbeing.state?.habits ?? [] as habit (habit.id)}
      <li class="pk-row" class:done={habit.doneToday} class:rest={!habit.scheduledToday}>
        {#if habit.target === 1}
          <button
            class="check"
            onclick={() => wellbeing.run(ipc.habitLog(habit.id, habit.doneToday ? -1 : 1))}
            aria-label={habit.doneToday
              ? t.wellbeing.undoCheckIn(habit.name)
              : t.wellbeing.checkIn(habit.name)}
          >
            {#if habit.doneToday}<Icon name="check" size={14} />{/if}
          </button>
        {:else}
          <span class="counter">
            <button
              class="pk-icon-button"
              onclick={() => wellbeing.run(ipc.habitLog(habit.id, -1))}
              disabled={habit.countToday === 0}
              aria-label={t.wellbeing.undoCheckIn(habit.name)}>−</button
            >
            <span class="count">{habit.countToday}/{habit.target}</span>
            <button
              class="pk-icon-button"
              onclick={() => wellbeing.run(ipc.habitLog(habit.id, 1))}
              aria-label={t.wellbeing.checkIn(habit.name)}>+</button
            >
          </span>
        {/if}
        <span class="pk-grow name">
          {habit.name}
          {#if !habit.scheduledToday}<small>{t.wellbeing.page.notToday}</small>{/if}
        </span>
        {#if habit.streak > 0}
          <span class="streak" title={t.wellbeing.streak(habit.streak)}>🔥 {habit.streak}</span>
        {/if}
      </li>
    {:else}
      <li class="pk-muted empty">{t.wellbeing.noHabits}</li>
    {/each}
  </ul>

  <button class="pk-button manage" onclick={onOpenPage}>{t.wellbeing.manageHabits} ›</button>
</div>

<style>
  .habits {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  ul {
    flex: 1;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    color: var(--island-bg);
    border: 1.5px solid var(--island-text-muted);
    border-radius: 6px;
  }

  .done .check {
    background: var(--accent);
    border-color: var(--accent);
  }

  .counter {
    display: flex;
    align-items: center;
  }

  .count {
    min-width: 30px;
    text-align: center;
    font: var(--text-mono);
  }

  /* A day off: still tappable (a bonus counts), just quieter. */
  .rest .name {
    color: var(--island-text-muted);
  }

  .name small {
    margin-left: var(--space-1);
    font: var(--text-caption);
  }

  .done .name {
    color: var(--island-text-muted);
  }

  .streak {
    font: var(--text-caption);
    color: var(--warning);
  }

  .manage {
    align-self: flex-start;
  }

  .empty {
    padding: var(--space-6) var(--space-3);
    text-align: center;
  }
</style>
