<script lang="ts">
  // Settings → Modules → Wellbeing: the roomy place for everything that needs choosing or
  // reviewing (reminder schedules, habits, water goal, screen time history). The island keeps
  // only a glance and one action. Changes save at once and the island follows via
  // `wellbeing:state`. All of it stays on this PC.
  import { onMount } from "svelte";
  import Icon from "../../lib/Icon.svelte";
  import { MOODS } from "../../lib/companion";
  import { formatDuration } from "../../lib/format";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { wellbeing } from "../../lib/stores/wellbeing.svelte";
  import type { WellbeingSettings } from "../../lib/types/generated/WellbeingSettings";

  const current = $derived(wellbeing.state);
  const settings = $derived(wellbeing.state?.settings);
  const habitsDone = $derived(current?.habits.filter((h) => h.doneToday).length ?? 0);
  const todaysMood = $derived(MOODS.find((m) => m.id === current?.moodToday));
  const topApp = $derived(Math.max(1, ...(current?.screenApps.map((a) => a.seconds) ?? [1])));
  const weekMax = $derived(Math.max(1, ...(current?.screenWeekS ?? [1])));
  /** Labels for the last 7 days, oldest first, matching `screenWeekS`. */
  const days = $derived.by(() => {
    const today = new Date();
    return Array.from({ length: 7 }, (_, i) => {
      const day = new Date(today);
      day.setDate(today.getDate() - (6 - i));
      return t.wellbeing.weekdays(day);
    });
  });

  let habitName = $state("");
  let habitTarget = $state(1);
  /** Weekday mask for the new habit: bit 0 = Monday … bit 6 = Sunday. */
  let habitDays = $state(127);

  function hasDay(mask: number, index: number): boolean {
    return (mask & (1 << index)) !== 0;
  }

  /** "every day", or "Mon, Wed, Fri". */
  function daysLabel(mask: number): string {
    if (mask === 127) return t.wellbeing.page.everyDay;
    return t.wellbeing.page.dayShort.filter((_, index) => hasDay(mask, index)).join(", ");
  }

  /** Flips one day of a habit's schedule; the last remaining day can't be removed. */
  function toggleHabitDay(
    habit: { id: string; name: string; target: number; days: number },
    index: number,
  ) {
    const next = habit.days ^ (1 << index);
    if (next === 0) return;
    void wellbeing.run(ipc.habitUpdate(habit.id, habit.name, habit.target, next));
  }
  let confirmingHabit = $state<string | null>(null);

  onMount(() => {
    let off: (() => void) | undefined;
    void wellbeing.connect().then((unlisten) => (off = unlisten));
    return () => off?.();
  });

  async function addHabit() {
    if (!habitName.trim()) return;
    const result = await wellbeing.run(ipc.habitAdd(habitName, habitTarget, habitDays || 127));
    if (result.ok) {
      habitName = "";
      habitTarget = 1;
      habitDays = 127;
    }
  }

  function save(change: Partial<WellbeingSettings>) {
    if (!settings) return;
    void wellbeing.run(ipc.wellbeingSettingsSet({ ...settings, ...change }));
  }

  type Toggle = "eyesEnabled" | "standEnabled" | "postureEnabled" | "screenBalanceEnabled";
  type Minutes = "eyesEveryMin" | "standEveryMin" | "postureEveryMin" | "screenBalanceMin";
  const breaks: Array<{
    label: string;
    toggle: Toggle;
    minutes: Minutes;
    unit: string;
    min: number;
    max: number;
  }> = [
    {
      label: t.wellbeing.eyes,
      toggle: "eyesEnabled",
      minutes: "eyesEveryMin",
      unit: t.wellbeing.every,
      min: 5,
      max: 240,
    },
    {
      label: t.wellbeing.stand,
      toggle: "standEnabled",
      minutes: "standEveryMin",
      unit: t.wellbeing.every,
      min: 5,
      max: 240,
    },
    {
      label: t.wellbeing.posture,
      toggle: "postureEnabled",
      minutes: "postureEveryMin",
      unit: t.wellbeing.every,
      min: 5,
      max: 240,
    },
    {
      label: t.wellbeing.screenBalance,
      toggle: "screenBalanceEnabled",
      minutes: "screenBalanceMin",
      unit: t.wellbeing.after,
      min: 30,
      max: 480,
    },
  ];

  function setMeal(index: number, value: string) {
    if (!settings) return;
    save({ mealTimes: settings.mealTimes.map((m, i) => (i === index ? value : m)) });
  }

  function removeMeal(index: number) {
    if (!settings) return;
    save({ mealTimes: settings.mealTimes.filter((_, i) => i !== index) });
  }
</script>

{#if current && settings}
  <div class="wellbeing">
    <!-- ---- Today ---- -->
    <section>
      <h3>{t.wellbeing.page.today}</h3>
      <div class="stats">
        <div class="stat">
          <span class="value">{current.waterMl} <small>ml</small></span>
          <span class="muted">{t.wellbeing.goal(settings.waterGoalMl)}</span>
          <span class="stat-actions">
            <button class="btn" onclick={() => wellbeing.run(ipc.waterAdd())}>
              {t.wellbeing.addGlass(settings.glassMl)}
            </button>
            <button
              class="icon-btn"
              onclick={() => wellbeing.run(ipc.waterUndo())}
              disabled={current.waterMl === 0}
              aria-label={t.wellbeing.undo}
              title={t.wellbeing.undo}
            >
              <Icon name="restart" size={14} />
            </button>
          </span>
        </div>
        <div class="stat">
          <span class="value">{current.breaksToday}</span>
          <span class="muted">{t.wellbeing.page.breaksTaken}</span>
          <span class="muted small">
            {current.nextBreakS !== null
              ? t.wellbeing.nextBreak(formatDuration(current.nextBreakS))
              : t.wellbeing.breaksOff}
          </span>
        </div>
        <div class="stat">
          <span class="value">{habitsDone} <small>/ {current.habits.length}</small></span>
          <span class="muted">{t.wellbeing.page.habitsToday}</span>
        </div>
        {#if settings.moodCheckInEnabled}
          <div class="stat">
            <span class="value">{todaysMood ? todaysMood.emoji : "–"}</span>
            <span class="muted">
              {todaysMood
                ? t.companion.moodToday(t.companion.moods[todaysMood.id] ?? "")
                : t.wellbeing.page.noMood}
            </span>
          </div>
        {/if}
      </div>
    </section>

    <!-- ---- Habits ---- -->
    <section>
      <h3>{t.wellbeing.tabs.habits}</h3>
      <ul class="list">
        {#each current.habits as habit (habit.id)}
          <li class="row">
            <span class="grow">
              <span class="name">{habit.name}</span>
              <small class="muted">
                {habit.scheduledToday || habit.countToday > 0
                  ? t.wellbeing.page.habitToday(habit.countToday, habit.target)
                  : t.wellbeing.page.notToday}
                · {daysLabel(habit.days)}
                {#if habit.streak > 0}· 🔥 {t.wellbeing.streak(habit.streak)}{/if}
                {#if habit.cameBack}· {t.wellbeing.page.welcomeBack} 👋{/if}
              </small>
            </span>
            <span class="days" role="group" aria-label={t.wellbeing.page.daysHint}>
              {#each t.wellbeing.page.dayLetters as letter, index (index)}
                <button
                  class="day-chip"
                  class:on={hasDay(habit.days, index)}
                  aria-pressed={hasDay(habit.days, index)}
                  aria-label={t.wellbeing.page.dayNames[index]}
                  title={t.wellbeing.page.dayNames[index]}
                  onclick={() => toggleHabitDay(habit, index)}>{letter}</button
                >
              {/each}
            </span>
            {#if confirmingHabit === habit.id}
              <small class="muted">{t.wellbeing.page.deleteHabitAsk}</small>
              <button
                class="btn danger"
                onclick={() => {
                  confirmingHabit = null;
                  void wellbeing.run(ipc.habitDelete(habit.id));
                }}>{t.wellbeing.deleteHabit}</button
              >
              <button class="btn" onclick={() => (confirmingHabit = null)}>
                {t.wellbeing.page.keep}
              </button>
            {:else}
              <button
                class="icon-btn"
                onclick={() => (confirmingHabit = habit.id)}
                aria-label="{t.wellbeing.deleteHabit}: {habit.name}"
                title={t.wellbeing.deleteHabit}
              >
                <Icon name="trash" size={14} />
              </button>
            {/if}
          </li>
        {:else}
          <li class="muted empty">{t.wellbeing.page.noHabitsYet}</li>
        {/each}
      </ul>
      <form
        class="add"
        onsubmit={(e) => {
          e.preventDefault();
          void addHabit();
        }}
      >
        <input
          class="field grow"
          bind:value={habitName}
          maxlength="60"
          placeholder={t.wellbeing.habitPlaceholder}
          aria-label={t.wellbeing.habitPlaceholder}
        />
        <select
          class="field"
          bind:value={habitTarget}
          aria-label={t.wellbeing.habitTarget}
          title={t.wellbeing.habitTarget}
        >
          {#each [1, 2, 3, 4, 5, 8] as n (n)}
            <option value={n}>{t.wellbeing.page.timesADay(n)}</option>
          {/each}
        </select>
        <button class="btn primary" type="submit" disabled={!habitName.trim()}>
          {t.wellbeing.addHabit}
        </button>
      </form>
      <div class="new-days">
        <span class="days" role="group" aria-label={t.wellbeing.page.daysHint}>
          {#each t.wellbeing.page.dayLetters as letter, index (index)}
            <button
              type="button"
              class="day-chip"
              class:on={hasDay(habitDays, index)}
              aria-pressed={hasDay(habitDays, index)}
              aria-label={t.wellbeing.page.dayNames[index]}
              title={t.wellbeing.page.dayNames[index]}
              onclick={() => {
                const next = habitDays ^ (1 << index);
                if (next !== 0) habitDays = next;
              }}>{letter}</button
            >
          {/each}
        </span>
        <small class="muted">{t.wellbeing.page.daysHint}</small>
      </div>
    </section>

    <!-- ---- Screen time ---- -->
    <section>
      <h3>{t.wellbeing.sections.screen}</h3>
      {#if !settings.screenTimeEnabled}
        <p class="muted">{t.wellbeing.screenOff}</p>
      {:else if !current.screenSupported}
        <p class="muted">{t.wellbeing.screenUnsupported}</p>
      {:else}
        <div class="screen">
          <div class="summary">
            <span class="muted">{t.wellbeing.screenTotal}</span>
            <span class="value">{formatDuration(current.screenTotalS)}</span>
            <div class="week" role="img" aria-label={t.wellbeing.screenWeek}>
              {#each current.screenWeekS as seconds, i (i)}
                <span class="day" title={formatDuration(seconds)}>
                  <span
                    class="bar"
                    class:today={i === 6}
                    style:height="{(seconds / weekMax) * 100}%"
                  ></span>
                  <span class="label">{days[i]}</span>
                </span>
              {/each}
            </div>
          </div>
          <ul class="apps">
            {#each current.screenApps as usage (usage.app)}
              <li>
                <span class="grow app">{usage.app}</span>
                <span class="meter"
                  ><span style:width="{(usage.seconds / topApp) * 100}%"></span></span
                >
                <span class="app-time">{formatDuration(usage.seconds)}</span>
              </li>
            {:else}
              <li class="muted">{t.wellbeing.screenEmpty}</li>
            {/each}
          </ul>
        </div>
      {/if}
    </section>

    <!-- ---- This week: plain counts, no scores ---- -->
    <section>
      <h3>{t.wellbeing.page.week}</h3>
      <p class="muted">{t.wellbeing.page.weekIntro}</p>
      <table class="reflection">
        <thead>
          <tr>
            <th scope="col"></th>
            {#each current.week as day (day.day)}
              <th scope="col">{t.wellbeing.weekdays(new Date(`${day.day}T12:00:00`))}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          <tr>
            <th scope="row">{t.wellbeing.page.weekWater}</th>
            {#each current.week as day (day.day)}
              <td>{day.waterMl > 0 ? (day.waterMl / 1000).toFixed(1) : "–"}</td>
            {/each}
          </tr>
          <tr>
            <th scope="row">{t.wellbeing.page.weekBreaks}</th>
            {#each current.week as day (day.day)}
              <td>{day.breaks || "–"}</td>
            {/each}
          </tr>
          <tr>
            <th scope="row">{t.wellbeing.page.weekHabits}</th>
            {#each current.week as day (day.day)}
              <td>{day.habitsDone || "–"}</td>
            {/each}
          </tr>
          {#if settings.moodCheckInEnabled}
            <tr>
              <th scope="row">{t.wellbeing.page.weekMood}</th>
              {#each current.week as day (day.day)}
                <td title={day.mood ? (t.companion.moods[day.mood] ?? "") : ""}>
                  {MOODS.find((m) => m.id === day.mood)?.emoji ?? "–"}
                </td>
              {/each}
            </tr>
          {/if}
        </tbody>
      </table>
      <p class="muted">
        {t.wellbeing.page.weekTotals(
          (current.week.reduce((sum, day) => sum + day.waterMl, 0) / 1000).toFixed(1),
          current.week.reduce((sum, day) => sum + day.breaks, 0),
          current.week.reduce((sum, day) => sum + day.habitsDone, 0),
        )}
      </p>
    </section>

    <!-- ---- Reminders (moved from the island) ---- -->
    <section class="reminders">
      <h3>{t.wellbeing.tabs.reminders}</h3>
      <h4>{t.wellbeing.sections.breaks}</h4>
      {#each breaks as row (row.toggle)}
        <label class="row">
          <input
            type="checkbox"
            role="switch"
            checked={settings[row.toggle]}
            onchange={(e) => save({ [row.toggle]: e.currentTarget.checked })}
          />
          <span class="grow">{row.label}</span>
          <span class="muted">{row.unit}</span>
          <input
            class="field num"
            type="number"
            min={row.min}
            max={row.max}
            value={settings[row.minutes]}
            disabled={!settings[row.toggle]}
            onchange={(e) => save({ [row.minutes]: Number(e.currentTarget.value) })}
            aria-label="{row.label} {row.unit}"
          />
        </label>
        {#if row.toggle === "standEnabled" && settings.standEnabled}
          <!-- Not everyone can (or wants to) stand up and walk. -->
          <label class="row sub">
            <input
              type="checkbox"
              role="switch"
              checked={settings.seatedMovement}
              onchange={(e) => save({ seatedMovement: e.currentTarget.checked })}
            />
            <span class="grow">{t.wellbeing.seated}</span>
          </label>
        {/if}
      {/each}

      <h4>{t.wellbeing.sections.water}</h4>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.waterEnabled}
          onchange={(e) => save({ waterEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.waterReminders}</span>
        <span class="muted">{t.wellbeing.every}</span>
        <input
          class="field num"
          type="number"
          min="5"
          max="240"
          value={settings.waterEveryMin}
          disabled={!settings.waterEnabled}
          onchange={(e) => save({ waterEveryMin: Number(e.currentTarget.value) })}
          aria-label="{t.wellbeing.waterReminders} {t.wellbeing.every}"
        />
      </label>
      <label class="row">
        <span class="grow">{t.wellbeing.waterGoal}</span>
        <input
          class="field num wide"
          type="number"
          min="500"
          max="6000"
          step="250"
          value={settings.waterGoalMl}
          onchange={(e) => save({ waterGoalMl: Number(e.currentTarget.value) })}
        />
      </label>
      <label class="row">
        <span class="grow">{t.wellbeing.glass}</span>
        <input
          class="field num wide"
          type="number"
          min="50"
          max="1000"
          step="50"
          value={settings.glassMl}
          onchange={(e) => save({ glassMl: Number(e.currentTarget.value) })}
        />
      </label>

      <h4>{t.wellbeing.sections.routine}</h4>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.bedtimeEnabled}
          onchange={(e) => save({ bedtimeEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.bedtime}</span>
        <input
          class="field time"
          type="time"
          value={settings.bedtime}
          disabled={!settings.bedtimeEnabled}
          onchange={(e) => save({ bedtime: e.currentTarget.value })}
          aria-label={t.wellbeing.bedtimeAt}
        />
      </label>
      {#if settings.bedtimeEnabled}
        <label class="row sub">
          <span class="grow">{t.wellbeing.windDown}</span>
          <input
            class="field num"
            type="number"
            min="0"
            max="120"
            step="5"
            value={settings.windDownMin}
            onchange={(e) => save({ windDownMin: Number(e.currentTarget.value) })}
          />
        </label>
      {/if}
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.mealsEnabled}
          onchange={(e) => save({ mealsEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.meals}</span>
      </label>
      {#if settings.mealsEnabled}
        <div class="row sub meals">
          {#each settings.mealTimes as meal, i (i)}
            <span class="meal">
              <input
                class="field time"
                type="time"
                value={meal}
                onchange={(e) => setMeal(i, e.currentTarget.value)}
                aria-label="{t.wellbeing.meals} {i + 1}"
              />
              <button
                class="icon-btn"
                onclick={() => removeMeal(i)}
                aria-label={t.wellbeing.removeMeal}
                title={t.wellbeing.removeMeal}>✕</button
              >
            </span>
          {/each}
          {#if settings.mealTimes.length < 4}
            <button
              class="btn"
              onclick={() => save({ mealTimes: [...settings.mealTimes, "19:30"] })}
            >
              + {t.wellbeing.addMeal}
            </button>
          {/if}
        </div>
      {/if}
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.moodCheckInEnabled}
          onchange={(e) => save({ moodCheckInEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.moodCheckIn}</span>
        <input
          class="field time"
          type="time"
          value={settings.moodCheckInTime}
          disabled={!settings.moodCheckInEnabled}
          onchange={(e) => save({ moodCheckInTime: e.currentTarget.value })}
          aria-label={t.wellbeing.moodCheckIn}
        />
      </label>
      <p class="muted note">{t.wellbeing.moodPrivacy}</p>

      <h4>{t.wellbeing.sections.prompts}</h4>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.randomNudges}
          onchange={(e) => save({ randomNudges: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.randomNudges}</span>
      </label>
      <p class="muted note">{t.wellbeing.randomNudgesNote}</p>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.quietHoursEnabled}
          onchange={(e) => save({ quietHoursEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.quietHours}</span>
        <input
          class="field time"
          type="time"
          value={settings.quietStart}
          disabled={!settings.quietHoursEnabled}
          onchange={(e) => save({ quietStart: e.currentTarget.value })}
          aria-label="{t.wellbeing.quietHours} {t.wellbeing.from}"
        />
        <span class="muted">{t.wellbeing.to}</span>
        <input
          class="field time"
          type="time"
          value={settings.quietEnd}
          disabled={!settings.quietHoursEnabled}
          onchange={(e) => save({ quietEnd: e.currentTarget.value })}
          aria-label="{t.wellbeing.quietHours} {t.wellbeing.to}"
        />
      </label>
      <label class="row">
        <span class="grow">{t.wellbeing.snooze}</span>
        <input
          class="field num"
          type="number"
          min="5"
          max="60"
          step="5"
          value={settings.snoozeMin}
          onchange={(e) => save({ snoozeMin: Number(e.currentTarget.value) })}
        />
      </label>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.encouragementEnabled}
          onchange={(e) => save({ encouragementEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.encouragement}</span>
      </label>
      <p class="muted note">{t.wellbeing.companionNote}</p>

      <h4>{t.wellbeing.sections.screen}</h4>
      <label class="row">
        <input
          type="checkbox"
          role="switch"
          checked={settings.screenTimeEnabled}
          onchange={(e) => save({ screenTimeEnabled: e.currentTarget.checked })}
        />
        <span class="grow">{t.wellbeing.screenTime}</span>
      </label>
      <p class="muted note">{t.wellbeing.screenPrivacy}</p>
    </section>
  </div>
{:else if wellbeing.error}
  <p class="error" role="alert">{wellbeing.error}</p>
{/if}

<style>
  .wellbeing {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  h3 {
    margin: 0;
    font: 700 15px / 20px var(--font-ui);
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

  .muted.small,
  small.muted {
    font-size: 12px;
    line-height: 16px;
  }

  .note {
    padding-left: 2px;
  }

  /* ---- Today ---- */
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: var(--space-2);
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .value {
    font: 700 22px / 28px var(--font-ui);
    font-variant-numeric: tabular-nums;
  }

  .value small {
    font: 500 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .stat-actions {
    display: flex;
    gap: var(--space-1);
    margin-top: var(--space-2);
  }

  /* ---- Rows ---- */
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    font: 500 14px / 20px var(--font-ui);
  }

  label.row:hover {
    background: var(--surface-2);
  }

  /* Sub-options line up under their parent's label. */
  .row.sub {
    padding-left: calc(var(--space-2) + 36px + var(--space-3));
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  .grow:has(.name) {
    display: flex;
    flex-direction: column;
  }

  .field {
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 6px var(--space-2);
  }

  .field:disabled {
    opacity: 0.5;
  }

  .num {
    width: 72px;
  }

  .time {
    width: 116px;
  }

  /* Same switch as the rest of Settings. */
  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 36px;
    height: 20px;
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
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--mod-wellbeing);
    border-color: var(--mod-wellbeing);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    transform: translateX(16px);
  }

  .num.wide {
    width: 96px;
  }

  .meals {
    flex-wrap: wrap;
  }

  .meal {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .btn {
    padding: 6px var(--space-3);
    font: var(--text-pill);
    color: var(--brand);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .btn {
    white-space: nowrap;
  }

  .btn.primary {
    color: #fff;
    background: var(--mod-wellbeing);
    border-color: var(--mod-wellbeing);
  }

  .btn.danger {
    color: #fff;
    background: var(--danger);
    border-color: var(--danger);
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
  }

  .icon-btn:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }

  .icon-btn:disabled {
    opacity: 0.4;
  }

  /* ---- Habits ---- */
  .list,
  .apps {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .list .row {
    background: var(--surface-2);
    margin-bottom: var(--space-1);
  }

  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .empty {
    padding: var(--space-2);
  }

  .add {
    display: flex;
    gap: var(--space-2);
  }

  .days {
    flex: none;
    display: inline-flex;
    gap: 2px;
  }

  .day-chip {
    width: 22px;
    height: 22px;
    font: 600 11px / 1 var(--font-ui);
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 50%;
  }

  .day-chip.on {
    color: #fff;
    background: var(--mod-wellbeing);
    border-color: var(--mod-wellbeing);
  }

  .new-days {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .reflection {
    width: 100%;
    border-collapse: collapse;
    font: 400 13px / 18px var(--font-ui);
    font-variant-numeric: tabular-nums;
  }

  .reflection th,
  .reflection td {
    padding: 6px 4px;
    text-align: center;
    border-bottom: 1px solid var(--border);
  }

  .reflection th {
    font-weight: 600;
    color: var(--text-muted);
  }

  .reflection th[scope="row"] {
    text-align: left;
    color: var(--text);
  }

  /* ---- Screen time ---- */
  .screen {
    display: flex;
    gap: var(--space-5);
    flex-wrap: wrap;
  }

  .summary {
    flex: none;
    width: 190px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .week {
    height: 80px;
    display: flex;
    align-items: flex-end;
    gap: 6px;
    margin-top: var(--space-2);
  }

  .day {
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    gap: 4px;
  }

  .bar {
    width: 100%;
    min-height: 2px;
    max-height: calc(100% - 16px);
    border-radius: 4px 4px 2px 2px;
    background: color-mix(in srgb, var(--mod-wellbeing) 40%, transparent);
  }

  .bar.today {
    background: var(--mod-wellbeing);
  }

  .label {
    font: 400 11px / 14px var(--font-ui);
    color: var(--text-muted);
  }

  .apps {
    flex: 1;
    min-width: 220px;
  }

  .apps li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 30px;
    font: 400 13px / 18px var(--font-ui);
  }

  .app {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .meter {
    flex: none;
    width: 90px;
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--surface-2);
    overflow: hidden;
  }

  .meter span {
    display: block;
    height: 100%;
    background: var(--mod-wellbeing);
    border-radius: inherit;
  }

  .app-time {
    flex: none;
    width: 64px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font: 500 13px / 18px var(--font-ui);
  }
</style>
