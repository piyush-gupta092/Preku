<script lang="ts">
  // The home view's "what now?" (usability U2): the next thing to do with one action, and a
  // factual recap of today. Counts of stored records only: no scores, no guesses about
  // distraction. Sections for modules that are off simply don't appear.
  import Icon from "../../lib/Icon.svelte";
  import { formatDuration } from "../../lib/format";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { productivity } from "../../lib/stores/productivity.svelte";
  import { wellbeing } from "../../lib/stores/wellbeing.svelte";
  import { clampMinutes, formatClock, formatFocusTotal, remainingSeconds } from "./tabs/helpers";
  import { nextAction, recapParts } from "./today";

  interface Props {
    now: number;
    /** Opens a Productivity tab (focus | tasks). */
    onopen: (tab: string) => void;
  }
  let { now, onopen }: Props = $props();

  const action = $derived(nextAction(productivity.timer, productivity.tasks));
  const recap = $derived(
    recapParts(productivity.today, wellbeing.state?.nextBreakS ?? null, {
      focused: (seconds) => t.today.focused(formatFocusTotal(seconds)),
      done: t.today.done,
      priorities: t.today.priorities,
      nextBreak: (seconds) => t.today.nextBreak(formatDuration(seconds)),
    }),
  );

  async function focus(id: string, title: string) {
    productivity.minutes = clampMinutes(productivity.minutes);
    const result = await productivity.run(ipc.timerStart(title, productivity.minutes, id));
    if (result.ok) onopen("focus");
  }
</script>

<section class="today" aria-label={t.today.title}>
  <div class="line">
    {#if action.kind === "focusing"}
      <span class="label">{action.paused ? t.home.paused : t.home.focusing}</span>
      <button class="what" onclick={() => onopen("focus")} title={action.title}>
        <span class="clock">{formatClock(remainingSeconds(productivity.timer, now))}</span>
        {action.title}
      </button>
      {#if action.paused}
        <button class="go" onclick={() => productivity.run(ipc.timerResume())}>
          <Icon name="play" size={14} />{t.focus.resume}
        </button>
      {/if}
    {:else if action.kind === "finished"}
      <span class="label">{t.focus.finished}</span>
      <button class="what" onclick={() => onopen("focus")} title={action.title}>
        {action.title}
      </button>
      <button class="go" onclick={() => onopen("focus")}>{t.today.choose}</button>
    {:else if action.kind === "task"}
      <span class="label">{action.priority ? t.today.priority : t.today.next}</span>
      <button class="what" onclick={() => onopen("tasks")} title={action.task.title}>
        {#if action.priority}<span class="star" aria-hidden="true">★</span>{/if}
        {action.task.title}
      </button>
      <button
        class="done"
        onclick={() => productivity.run(ipc.taskSetDone(action.task.id, true))}
        aria-label={t.tasks.markDone(action.task.title)}
        title={t.tasks.markDone(action.task.title)}
      >
        <Icon name="check" size={14} />
      </button>
      <button class="go" onclick={() => focus(action.task.id, action.task.title)}>
        <Icon name="play" size={14} />{t.today.focus}
      </button>
    {:else}
      <span class="label">{t.today.title}</span>
      <button class="what muted" onclick={() => onopen("tasks")}>{t.today.empty}</button>
      <button class="go" onclick={() => onopen("tasks")}>
        <Icon name="plus" size={14} />{t.today.addTask}
      </button>
    {/if}
  </div>
  {#if recap.length > 0}
    <p class="recap">{recap.join(" · ")}</p>
  {/if}
</section>

<style>
  .today {
    --accent: var(--mod-productivity);
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    min-height: 56px;
    padding: var(--space-2) var(--space-3);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .label {
    flex: none;
    font: var(--text-caption);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    text-align: left;
    font: var(--text-body);
    font-weight: 600;
    color: var(--island-text);
  }

  .what.muted {
    font-weight: 400;
    color: var(--island-text-muted);
  }

  .clock {
    margin-right: var(--space-1);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--accent);
  }

  .star {
    color: var(--accent);
  }

  .go,
  .done {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .go {
    padding: 0 var(--space-3);
    color: var(--island-bg);
    background: var(--accent);
  }

  .done {
    justify-content: center;
    width: 28px;
    color: var(--island-text-muted);
    border: 1px solid var(--island-border);
  }

  .done:hover {
    color: var(--island-text);
    background: var(--island-bg);
  }

  .recap {
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }
</style>
