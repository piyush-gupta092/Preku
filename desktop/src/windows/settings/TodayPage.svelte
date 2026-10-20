<script lang="ts">
  // The larger "Today" page (usability U6): the same tasks, recap and clipboard as the notch,
  // with room to edit. It shares the notch's state and commands, so both always agree, and
  // everything here works with the keyboard alone (real inputs and buttons, in reading order).
  import { onMount } from "svelte";
  import Icon from "../../lib/Icon.svelte";
  import { dueLabel, fromLocalInput, toLocalInput } from "../../lib/due";
  import { formatDuration } from "../../lib/format";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { productivity } from "../../lib/stores/productivity.svelte";
  import { wellbeing } from "../../lib/stores/wellbeing.svelte";
  import type { ClipItem } from "../../lib/types/generated/ClipItem";
  import type { Task } from "../../lib/types/generated/Task";
  import { clampMinutes, filterClips, formatFocusTotal, moveId } from "../island/tabs/helpers";
  import { recapParts } from "../island/today";
  import { thoughtFor } from "../../lib/quotes";
  import { removeThought, saveThought } from "../../lib/moment";
  import { events } from "../../lib/ipc";
  import { listen } from "@tauri-apps/api/event";
  import type { Preferences } from "../../lib/types/generated/Preferences";

  let draft = $state("");
  let query = $state("");
  let copied = $state<string | null>(null);
  let showDone = $state(false);
  let now = $state(Date.now());
  /** For the saved thoughts (the list lives in Preferences, shared with the notch). */
  let prefs = $state<Preferences | null>(null);
  const todaysThought = $derived(thoughtFor(new Date(now)));

  function setSaved(savedThoughts: string[]) {
    if (!prefs) return;
    prefs = { ...prefs, savedThoughts };
    void ipc.preferencesSet(prefs);
  }

  const open = $derived(productivity.tasks.filter((task) => !task.done));
  const done = $derived(productivity.tasks.filter((task) => task.done));
  const clips = $derived(filterClips(productivity.clipboard, query));
  const recap = $derived(
    recapParts(productivity.today, wellbeing.state?.nextBreakS ?? null, {
      focused: (seconds) => t.today.focused(formatFocusTotal(seconds)),
      done: t.today.done,
      priorities: t.today.priorities,
      nextBreak: (seconds) => t.today.nextBreak(formatDuration(seconds)),
    }),
  );

  onMount(() => {
    const offs: Array<() => void> = [];
    void productivity.connect(() => {}).then((off) => offs.push(off));
    void wellbeing.connect().then((off) => offs.push(off));
    void ipc.preferencesGet().then((result) => {
      if (result.ok) prefs = result.value;
    });
    void listen<Preferences>(events.preferences, (e) => (prefs = e.payload)).then((off) =>
      offs.push(off),
    );
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      clearInterval(timer);
      offs.forEach((off) => off());
    };
  });

  async function add() {
    if (!draft.trim()) return;
    const result = await productivity.run(ipc.taskAdd(draft));
    if (result.ok) draft = "";
  }

  function rename(task: Task, title: string) {
    if (title.trim() && title.trim() !== task.title) {
      void productivity.run(ipc.taskRename(task.id, title));
    }
  }

  function move(id: string, step: -1 | 1) {
    const ids = open.map((task) => task.id);
    const next = moveId(ids, id, step);
    if (next !== ids) void productivity.run(ipc.tasksReorder(next));
  }

  function due(task: Task) {
    return task.dueAt === null
      ? null
      : dueLabel(task.dueAt, now, {
          overdue: t.tasks.overdue,
          today: t.tasks.dueToday,
          tomorrow: t.tasks.dueTomorrow,
          later: t.tasks.dueLater,
        });
  }

  function focusOn(task: Task) {
    productivity.minutes = clampMinutes(productivity.minutes);
    void productivity.run(ipc.timerStart(task.title, productivity.minutes, task.id));
  }

  async function copy(item: ClipItem) {
    const result = await productivity.run(ipc.clipboardCopy(item.id));
    if (!result.ok) return;
    copied = item.id;
    setTimeout(() => (copied = null), 1200);
  }
</script>

<div class="today">
  {#if productivity.error}
    <p class="notice" role="status">{productivity.error}</p>
  {:else}
    <section class="card">
      <h2>{t.todayPage.recap}</h2>
      <p class="recap">{recap.length > 0 ? recap.join(" · ") : t.todayPage.nothingYet}</p>
      <p class="muted thought">
        “{todaysThought}”
        {#if prefs && !prefs.savedThoughts.includes(todaysThought)}
          <button
            class="link"
            onclick={() => setSaved(saveThought(prefs?.savedThoughts ?? [], todaysThought))}
          >
            ♡ {t.moment.save}
          </button>
        {/if}
      </p>
      {#if productivity.timer.status !== "idle"}
        <p class="muted">
          {productivity.timer.status === "paused" ? t.home.paused : t.home.focusing}:
          <b>{productivity.timer.label}</b>
        </p>
      {/if}
    </section>

    <section class="card">
      <h2>{t.tabs.tasks}</h2>
      <p class="muted">{t.todayPage.tasksHint}</p>
      <form
        class="add"
        onsubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          class="field grow"
          bind:value={draft}
          maxlength="200"
          placeholder={t.tasks.addPlaceholder}
          aria-label={t.tasks.addPlaceholder}
        />
        <button class="primary" type="submit" disabled={!draft.trim()}>{t.tasks.add}</button>
      </form>

      <ul>
        {#each open as task, index (task.id)}
          {@const label = due(task)}
          <li>
            <button
              class="check"
              onclick={() => productivity.run(ipc.taskSetDone(task.id, true))}
              aria-label={t.tasks.markDone(task.title)}
            ></button>
            <button
              class="star"
              class:on={task.priorityToday}
              aria-pressed={task.priorityToday}
              aria-label={task.priorityToday
                ? t.tasks.unprioritize(task.title)
                : t.tasks.prioritize(task.title)}
              title={task.priorityToday
                ? t.tasks.unprioritize(task.title)
                : t.tasks.prioritize(task.title)}
              onclick={() => productivity.run(ipc.taskSetPriority(task.id, !task.priorityToday))}
              >{task.priorityToday ? "★" : "☆"}</button
            >
            <span class="grow body">
              <input
                class="title"
                value={task.title}
                maxlength="200"
                aria-label={t.todayPage.taskTitle}
                onchange={(e) => rename(task, e.currentTarget.value)}
              />
              <small>
                {#if task.focusS > 0}
                  {t.tasks.focusStats(task.focusSessions, formatFocusTotal(task.focusS))}
                {/if}
                {#if label}
                  <span class:overdue={label.overdue}>{label.text}</span>
                {/if}
              </small>
              <span class="when-row">
                <span class="when-label">⏰</span>
                <input
                  class="field when"
                  type="datetime-local"
                  value={task.dueAt === null ? "" : toLocalInput(task.dueAt)}
                  aria-label={t.tasks.remindAt(task.title)}
                  title={t.tasks.remindAt(task.title)}
                  onchange={(e) =>
                    productivity.run(
                      ipc.taskSetDue(task.id, fromLocalInput(e.currentTarget.value)),
                    )}
                />
              </span>
            </span>
            <button
              class="icon"
              onclick={() => focusOn(task)}
              aria-label={t.tasks.focusOn(task.title)}
              title={t.tasks.focusOn(task.title)}
            >
              <Icon name="play" size={14} />
            </button>
            <button
              class="icon"
              onclick={() => move(task.id, -1)}
              disabled={index === 0}
              aria-label={t.tasks.moveUp}
            >
              <Icon name="up" size={14} />
            </button>
            <button
              class="icon"
              onclick={() => move(task.id, 1)}
              disabled={index === open.length - 1}
              aria-label={t.tasks.moveDown}
            >
              <Icon name="down" size={14} />
            </button>
            <button
              class="icon"
              onclick={() => productivity.run(ipc.taskDelete(task.id))}
              aria-label="{t.tasks.delete}: {task.title}"
            >
              <Icon name="trash" size={14} />
            </button>
          </li>
        {:else}
          <li class="muted">{t.tasks.empty}</li>
        {/each}
      </ul>

      {#if done.length > 0}
        <button class="link" aria-expanded={showDone} onclick={() => (showDone = !showDone)}>
          {t.todayPage.finished(done.length)}
        </button>
        {#if showDone}
          <ul>
            {#each done as task (task.id)}
              <li class="done">
                <button
                  class="check on"
                  onclick={() => productivity.run(ipc.taskSetDone(task.id, false))}
                  aria-label={t.tasks.markOpen(task.title)}
                >
                  <Icon name="check" size={12} />
                </button>
                <span class="grow title-done">{task.title}</span>
                <button
                  class="icon"
                  onclick={() => productivity.run(ipc.taskDelete(task.id))}
                  aria-label="{t.tasks.delete}: {task.title}"
                >
                  <Icon name="trash" size={14} />
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>

    <section class="card">
      <h2>{t.tabs.clipboard}</h2>
      <p class="muted">{t.clipboard.privacy}</p>
      <input
        class="field"
        type="search"
        bind:value={query}
        placeholder={t.clipboard.search}
        aria-label={t.clipboard.search}
      />
      <ul class="clips">
        {#each clips as item (item.id)}
          <li>
            <span class="grow clip" title={item.kind === "text" ? item.preview : ""}>
              {item.kind === "text" ? item.preview : t.clipboard.image(item.width, item.height)}
            </span>
            <button class="ghost" onclick={() => copy(item)}>
              {copied === item.id ? t.clipboard.copied : t.clipboard.copy}
            </button>
            <button
              class="ghost"
              aria-pressed={item.pinned}
              onclick={() => productivity.run(ipc.clipboardSetPinned(item.id, !item.pinned))}
            >
              {item.pinned ? t.clipboard.unpin : t.clipboard.pin}
            </button>
            <button
              class="icon"
              onclick={() => productivity.run(ipc.clipboardDelete(item.id))}
              aria-label={t.clipboard.delete}
            >
              <Icon name="trash" size={14} />
            </button>
          </li>
        {:else}
          <li class="muted">{query.trim() ? t.clipboard.noMatch : t.clipboard.empty}</li>
        {/each}
      </ul>
    </section>
    <section class="card">
      <h2>{t.moment.savedTitle}</h2>
      <ul>
        {#each prefs?.savedThoughts ?? [] as saved (saved)}
          <li>
            <span class="grow saved">“{saved}”</span>
            <button
              class="ghost"
              onclick={() => setSaved(removeThought(prefs?.savedThoughts ?? [], saved))}
            >
              {t.moment.remove}
            </button>
          </li>
        {:else}
          <li class="muted">{t.moment.savedEmpty}</li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .today {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 20px 24px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  h2 {
    margin: 0;
    font: 700 16px / 22px var(--font-ui);
  }

  .recap {
    margin: 0;
    font: 600 15px / 22px var(--font-ui);
  }

  .muted,
  .notice {
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

  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 44px;
    padding: var(--space-1) var(--space-2);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  li.muted {
    background: none;
  }

  .add {
    display: flex;
    gap: var(--space-2);
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  .body {
    display: flex;
    flex-direction: column;
  }

  .field {
    padding: 6px var(--space-2);
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .when-row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding-left: 5px;
  }

  .when-label {
    font-size: 12px;
  }

  .when {
    width: 190px;
    padding: 2px var(--space-2);
    font-size: 12px;
  }

  .title {
    width: 100%;
    padding: 2px 4px;
    font: 500 14px / 20px var(--font-ui);
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
  }

  .title:hover,
  .title:focus-visible {
    background: var(--surface);
    border-color: var(--border);
  }

  small {
    display: flex;
    gap: var(--space-2);
    padding-left: 5px;
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .overdue {
    color: var(--danger);
  }

  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    color: #fff;
    border: 1.5px solid var(--text-muted);
    border-radius: 6px;
  }

  .check.on {
    background: var(--mod-productivity);
    border-color: var(--mod-productivity);
  }

  .star {
    flex: none;
    width: 22px;
    font: 400 16px / 1 var(--font-ui);
    color: var(--text-muted);
  }

  .star.on,
  .star:hover {
    color: var(--mod-productivity);
  }

  .title-done {
    color: var(--text-muted);
    text-decoration: line-through;
  }

  .icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
  }

  .icon:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface);
  }

  .icon:disabled {
    opacity: 0.3;
  }

  .primary,
  .ghost {
    flex: none;
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
  }

  .ghost {
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .link {
    align-self: flex-start;
    padding: 0;
    color: var(--brand);
    font: 600 13px / 18px var(--font-ui);
  }

  .saved {
    padding: var(--space-1) 0;
    font: 500 14px / 20px var(--font-ui);
  }

  .thought .link {
    display: inline;
    margin-left: var(--space-2);
  }

  .clips {
    max-height: 320px;
    overflow-y: auto;
  }

  .clip {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: 400 13px / 18px var(--font-mono);
  }
</style>
