<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { dueChoices, dueLabel } from "../../../lib/due";
  import { productivity } from "../../../lib/stores/productivity.svelte";
  import { clampMinutes, focusOnAction, formatFocusTotal, moveId } from "./helpers";

  interface Props {
    /** Opens the Focus tab after "Focus on this" started a session. */
    onfocus?: () => void;
    /** Changes when quick capture is pressed: the cursor goes to "add a task". */
    captureKey?: number;
  }
  let { onfocus, captureKey = 0 }: Props = $props();

  let addField = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (captureKey > 0) addField?.focus();
  });

  /** The task whose reminder menu is open. */
  let reminding = $state<string | null>(null);
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(timer);
  });

  function setDue(id: string, at: number | null) {
    reminding = null;
    void productivity.run(ipc.taskSetDue(id, at));
  }

  function due(dueAt: number | null) {
    return dueAt === null
      ? null
      : dueLabel(dueAt, now, {
          overdue: t.tasks.overdue,
          today: t.tasks.dueToday,
          tomorrow: t.tasks.dueTomorrow,
          later: t.tasks.dueLater,
        });
  }

  let draft = $state("");
  let editing = $state<string | null>(null);
  let editText = $state("");
  /** The task waiting for "end the current session and switch?" */
  let switching = $state<string | null>(null);

  const open = $derived(productivity.tasks.filter((task) => !task.done));
  const done = $derived(productivity.tasks.filter((task) => task.done));

  async function add() {
    const title = draft.trim();
    if (!title) return;
    const result = await productivity.run(ipc.taskAdd(title));
    if (result.ok) draft = "";
  }

  function startEdit(id: string, title: string) {
    editing = id;
    editText = title;
  }

  function commitEdit() {
    const id = editing;
    editing = null;
    if (id && editText.trim()) void productivity.run(ipc.taskRename(id, editText));
  }

  function move(id: string, step: -1 | 1) {
    const ids = open.map((task) => task.id);
    const next = moveId(ids, id, step);
    if (next !== ids) void productivity.run(ipc.tasksReorder(next));
  }

  async function focusOn(id: string, title: string, confirmed = false) {
    const action = focusOnAction(productivity.timer, id);
    if (action === "confirm" && !confirmed) {
      switching = id;
      return;
    }
    switching = null;
    if (action === "open") return onfocus?.();
    if (action === "resume") {
      const result = await productivity.run(ipc.timerResume());
      if (result.ok) onfocus?.();
      return;
    }
    // Same rules as the Focus tab: an empty or odd custom length becomes a valid one.
    productivity.minutes = clampMinutes(productivity.minutes);
    const result = await productivity.run(ipc.timerStart(title, productivity.minutes, id));
    if (result.ok) onfocus?.();
  }

  /** Focuses the inline rename field as soon as it renders. */
  function autofocus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<div class="tasks">
  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <input
      bind:this={addField}
      bind:value={draft}
      maxlength="200"
      placeholder={t.tasks.addPlaceholder}
      aria-label={t.tasks.addPlaceholder}
    />
    <button type="submit" aria-label={t.tasks.add} disabled={!draft.trim()}>
      <Icon name="plus" size={18} />
    </button>
  </form>

  <ul class="list">
    {#each [...open, ...done] as task, index (task.id)}
      <li class:done={task.done}>
        <button
          class="check"
          onclick={() => productivity.run(ipc.taskSetDone(task.id, !task.done))}
          aria-label={task.done ? t.tasks.markOpen(task.title) : t.tasks.markDone(task.title)}
        >
          {#if task.done}<Icon name="check" size={14} />{/if}
        </button>

        {#if !task.done}
          <button
            class="star"
            class:on={task.priorityToday}
            onclick={() => productivity.run(ipc.taskSetPriority(task.id, !task.priorityToday))}
            aria-pressed={task.priorityToday}
            aria-label={task.priorityToday
              ? t.tasks.unprioritize(task.title)
              : t.tasks.prioritize(task.title)}
            title={task.priorityToday
              ? t.tasks.unprioritize(task.title)
              : t.tasks.prioritize(task.title)}>{task.priorityToday ? "★" : "☆"}</button
          >
        {/if}

        {#if editing === task.id}
          <input
            class="edit"
            bind:value={editText}
            maxlength="200"
            use:autofocus
            onblur={commitEdit}
            onkeydown={(e) => {
              if (e.key === "Enter") commitEdit();
              if (e.key === "Escape") {
                e.stopPropagation();
                editing = null;
              }
            }}
          />
        {:else}
          <span class="text">
            <span
              class="title"
              role="button"
              tabindex="0"
              title={t.tasks.rename}
              ondblclick={() => startEdit(task.id, task.title)}
              onkeydown={(e) => e.key === "F2" && startEdit(task.id, task.title)}
            >
              {task.title}
            </span>
            {#if productivity.timer.taskId === task.id && productivity.timer.status !== "idle"}
              <small class="now">{t.tasks.focusingNow}</small>
            {:else if !task.done && due(task.dueAt)}
              {@const label = due(task.dueAt)}
              <small class:late={label?.overdue}>⏰ {label?.text}</small>
            {:else if task.focusS > 0}
              <small>{t.tasks.focusStats(task.focusSessions, formatFocusTotal(task.focusS))}</small>
            {/if}
          </span>
        {/if}

        {#if switching === task.id}
          <span
            class="switch"
            role="group"
            aria-label={t.tasks.switchAsk(productivity.timer.label)}
          >
            <small>{t.tasks.switchAsk(productivity.timer.label)}</small>
            <button class="yes" onclick={() => focusOn(task.id, task.title, true)}>
              {t.tasks.switch}
            </button>
            <button onclick={() => (switching = null)}>{t.tasks.keep}</button>
          </span>
        {:else if !task.done}
          <button
            class="focus-on"
            onclick={() => focusOn(task.id, task.title)}
            aria-label={t.tasks.focusOn(task.title)}
            title={t.tasks.focusOn(task.title)}
          >
            <Icon name="play" size={14} />
          </button>
        {/if}

        {#if reminding === task.id}
          <span class="remind-menu" role="menu" aria-label={t.tasks.remind(task.title)}>
            {#if task.dueAt !== null && task.dueAt <= now}
              <button role="menuitem" onclick={() => setDue(task.id, Date.now() + 10 * 60_000)}>
                {t.tasks.remindLater}
              </button>
            {/if}
            {#each dueChoices(now) as choice (choice.id)}
              <button role="menuitem" onclick={() => setDue(task.id, choice.at)}>
                {t.tasks.remindChoices[choice.id]}
              </button>
            {/each}
            {#if task.dueAt !== null}
              <button role="menuitem" onclick={() => setDue(task.id, null)}>
                {t.tasks.remindClear}
              </button>
            {/if}
          </span>
        {/if}

        <span class="actions">
          {#if !task.done}
            <button
              onclick={() => (reminding = reminding === task.id ? null : task.id)}
              aria-expanded={reminding === task.id}
              aria-label={t.tasks.remind(task.title)}
              title={t.tasks.remind(task.title)}
            >
              <Icon name="clock" size={14} />
            </button>
            <button
              onclick={() => move(task.id, -1)}
              disabled={index === 0}
              aria-label={t.tasks.moveUp}
            >
              <Icon name="up" size={14} />
            </button>
            <button
              onclick={() => move(task.id, 1)}
              disabled={index === open.length - 1}
              aria-label={t.tasks.moveDown}
            >
              <Icon name="down" size={14} />
            </button>
          {/if}
          <button
            onclick={() => productivity.run(ipc.taskDelete(task.id))}
            aria-label={t.tasks.delete}
          >
            <Icon name="trash" size={14} />
          </button>
        </span>
      </li>
    {:else}
      <li class="empty">{t.tasks.empty}</li>
    {/each}
  </ul>
</div>

<style>
  .tasks {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  .add {
    display: flex;
    gap: var(--space-2);
  }

  input {
    flex: 1;
    min-width: 0;
    font: var(--text-body);
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
  }

  input::placeholder {
    color: var(--island-text-muted);
  }

  input:focus-visible {
    outline: none;
    border-color: var(--mod-productivity);
  }

  .add button {
    display: grid;
    place-items: center;
    width: 36px;
    color: var(--island-bg);
    background: var(--mod-productivity);
    border-radius: var(--radius-sm);
  }

  .add button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .list {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-y: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: 0 var(--space-1);
    border-radius: var(--radius-sm);
  }

  li:hover {
    background: var(--island-bg-raised);
  }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    color: var(--island-bg);
    border: 1.5px solid var(--island-text-muted);
    border-radius: 6px;
  }

  /* Today's priorities (up to three): a filled star, and the task moves to the top. */
  .star {
    flex: none;
    width: 20px;
    font: 400 15px / 1 var(--font-ui);
    color: var(--island-text-muted);
    opacity: 0.55;
  }

  .star:hover,
  .star.on {
    color: var(--mod-productivity);
    opacity: 1;
  }

  .done .check {
    background: var(--mod-productivity);
    border-color: var(--mod-productivity);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    padding: var(--space-1) 0;
  }

  .text small {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .text small.late {
    color: var(--danger);
  }

  /* Reminder choices, on their own line under the task. */
  li {
    flex-wrap: wrap;
  }

  .remind-menu {
    order: 2;
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    padding: 0 0 var(--space-2) 46px;
  }

  .remind-menu button {
    padding: 3px 10px;
    font: var(--text-pill);
    color: var(--island-text);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .remind-menu button:hover {
    background: var(--island-bg);
  }

  .text small.now {
    color: var(--mod-productivity);
  }

  .title {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    cursor: text;
  }

  .done .title {
    color: var(--island-text-muted);
    text-decoration: line-through;
  }

  .edit {
    padding: var(--space-1) var(--space-2);
  }

  .actions {
    display: flex;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  li:hover .actions,
  li:focus-within .actions {
    opacity: 1;
  }

  .actions button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  /* Always visible, at the far right; the hover actions open to its left. */
  .focus-on {
    order: 1;
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--mod-productivity);
    border-radius: var(--radius-sm);
  }

  .switch {
    order: 1;
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    max-width: 60%;
  }

  .switch small {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .switch button {
    flex: none;
    height: 26px;
    padding: 0 var(--space-2);
    font: var(--text-pill);
    color: var(--island-text);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .switch button.yes {
    color: var(--island-bg);
    background: var(--mod-productivity);
    border-color: var(--mod-productivity);
  }

  .focus-on:hover {
    background: var(--island-bg);
  }

  .actions button:hover:not(:disabled) {
    color: var(--island-text);
    background: var(--island-bg);
  }

  .actions button:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .empty {
    justify-content: center;
    min-height: 120px;
    color: var(--island-text-muted);
    font: var(--text-caption);
  }

  .empty:hover {
    background: none;
  }
</style>
