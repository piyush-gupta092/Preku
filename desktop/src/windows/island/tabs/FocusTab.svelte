<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../../lib/Icon.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { companion } from "../../../lib/stores/companion.svelte";
  import { productivity } from "../../../lib/stores/productivity.svelte";
  import { wellbeing } from "../../../lib/stores/wellbeing.svelte";
  import Ring from "./Ring.svelte";
  import {
    clampMinutes,
    formatClock,
    formatFocusTotal,
    progress,
    remainingSeconds,
  } from "./helpers";

  const presets = [15, 25, 50];

  let label = $state("");
  /** Optional task to focus on: its title becomes the label. */
  let taskId = $state("");
  let now = $state(Date.now());
  const timer = $derived(productivity.timer);
  const idle = $derived(timer.status === "idle");
  const openTasks = $derived(productivity.tasks.filter((task) => !task.done));
  const finished = $derived(idle ? timer.finished : null);
  /** "Mark done" only while that task is still open. */
  const finishedTask = $derived(
    finished?.taskId ? (openTasks.find((task) => task.id === finished.taskId) ?? null) : null,
  );
  // A task that got done or deleted elsewhere drops out of the picker.
  $effect(() => {
    if (taskId && !openTasks.some((task) => task.id === taskId)) taskId = "";
  });

  onMount(() => {
    // 4 fps is plenty for a seconds display and keeps CPU low (Rules.md §5).
    const id = setInterval(() => (now = Date.now()), 250);
    return () => clearInterval(id);
  });

  function start() {
    productivity.minutes = clampMinutes(productivity.minutes);
    void productivity.run(ipc.timerStart(label, productivity.minutes, taskId || null));
  }

  function continueFocus() {
    if (!finished) return;
    const minutes = clampMinutes(Math.round(finished.totalS / 60));
    void productivity.run(ipc.timerStart(finished.label, minutes, finishedTask?.id ?? null));
  }

  async function takeBreak() {
    await productivity.run(ipc.timerDismissFinished());
    // The island goes back home, where the companion card guides the break.
    void companion.take("stand");
  }
</script>

<div class="focus">
  <Ring size={148} value={finished ? 1 : idle ? 0 : progress(timer, now)} stroke={8}>
    <div class="readout">
      {#if finished}
        <Icon name="check" size={36} />
      {:else}
        <span class="time"
          >{formatClock(idle ? productivity.minutes * 60 : remainingSeconds(timer, now))}</span
        >
      {/if}
      <span class="state">
        {#if finished}{formatFocusTotal(finished.focusedS)}{:else if idle}{t.focus
            .ready}{:else if timer.status === "paused"}{t.home.paused}{:else}{t.focus.left}{/if}
      </span>
    </div>
  </Ring>

  <div class="side">
    {#if finished}
      <div class="finished">
        <span class="eyebrow">{t.focus.finished}</span>
        <button
          class="close"
          onclick={() => productivity.run(ipc.timerDismissFinished())}
          aria-label={t.focus.close}
        >
          <Icon name="close" size={14} />
        </button>
      </div>
      <span class="current">{finished.label}</span>
      <div class="choices">
        {#if finishedTask}
          <button
            class="primary"
            onclick={() => productivity.run(ipc.taskSetDone(finishedTask.id, true))}
          >
            <Icon name="check" size={16} />
            {t.focus.markDone}
          </button>
        {/if}
        <div class="actions">
          <button class={finishedTask ? "secondary" : "primary"} onclick={continueFocus}>
            <Icon name="play" size={16} />
            {t.focus.continue(clampMinutes(Math.round(finished.totalS / 60)))}
          </button>
          {#if wellbeing.state}
            <button class="secondary" onclick={takeBreak}>{t.focus.takeBreak}</button>
          {/if}
        </div>
      </div>
    {:else if idle}
      {#if openTasks.length > 0}
        <select class="task" bind:value={taskId} aria-label={t.focus.task}>
          <option value="">{t.focus.noTask}</option>
          {#each openTasks as task (task.id)}
            <option value={task.id}>{task.title}</option>
          {/each}
        </select>
      {/if}
      {#if !taskId}
        <input
          class="label"
          bind:value={label}
          maxlength="60"
          placeholder={t.focus.labelPlaceholder}
          aria-label={t.focus.labelPlaceholder}
          onkeydown={(e) => e.key === "Enter" && start()}
        />
      {/if}
      <div class="presets" role="group" aria-label={t.focus.minutes}>
        {#each presets as preset (preset)}
          <button
            class:active={productivity.minutes === preset}
            onclick={() => (productivity.minutes = preset)}
          >
            {t.focus.preset(preset)}
          </button>
        {/each}
        <input
          class="custom"
          type="number"
          min="1"
          max="180"
          bind:value={productivity.minutes}
          aria-label={t.focus.customMinutes}
        />
      </div>
      <button class="primary" onclick={start}>
        <Icon name="play" size={16} />
        {t.focus.start}
      </button>
    {:else}
      <span class="current">{timer.label}</span>
      <div class="actions">
        {#if timer.status === "running"}
          <button class="primary" onclick={() => productivity.run(ipc.timerPause())}>
            <Icon name="pause" size={16} />
            {t.focus.pause}
          </button>
        {:else}
          <button class="primary" onclick={() => productivity.run(ipc.timerResume())}>
            <Icon name="play" size={16} />
            {t.focus.resume}
          </button>
        {/if}
        <button class="secondary" onclick={() => productivity.run(ipc.timerStop())}>
          <Icon name="stop" size={16} />
          {t.focus.stop}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .focus {
    --accent: var(--mod-productivity);
    display: flex;
    align-items: center;
    gap: var(--space-5);
    height: 100%;
    padding: 0 var(--space-2);
  }

  .readout {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .time {
    font: var(--text-display);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .state {
    font: var(--text-caption);
    color: var(--island-text-muted);
    text-transform: lowercase;
  }

  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  input {
    font: var(--text-body);
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
  }

  select.task {
    font: var(--text-body);
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
    min-width: 0;
  }

  select.task:focus-visible {
    outline: none;
    border-color: var(--mod-productivity);
  }

  .finished {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .eyebrow {
    font: var(--text-caption);
    color: var(--mod-productivity);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .close:hover {
    color: var(--island-text);
    background: var(--island-bg-raised);
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  input::placeholder {
    color: var(--island-text-muted);
  }

  input:focus-visible {
    outline: none;
    border-color: var(--mod-productivity);
  }

  .presets {
    display: flex;
    gap: var(--space-2);
  }

  .presets button {
    flex: 1;
    height: 32px;
    font: var(--text-pill);
    color: var(--island-text-muted);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
  }

  .presets button.active {
    color: var(--island-bg);
    background: var(--mod-productivity);
    border-color: var(--mod-productivity);
  }

  .custom {
    width: 64px;
    padding: 0 var(--space-2);
    text-align: center;
  }

  .current {
    font: var(--text-title);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    gap: var(--space-2);
  }

  .actions > button {
    flex: 1;
  }

  .primary,
  .secondary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    height: 36px;
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .primary {
    color: var(--island-bg);
    background: var(--mod-productivity);
  }

  .secondary {
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
  }

  .primary:hover,
  .secondary:hover {
    opacity: 0.9;
  }
</style>
