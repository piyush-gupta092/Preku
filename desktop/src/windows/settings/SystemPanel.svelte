<script lang="ts">
  // Settings → Modules → System: the alert rules, with room to read and edit them (they were
  // moved here from the notch, where the editor overflowed). A rule says what to watch, the
  // limit, and how long it must hold before Preku speaks up, so brief spikes stay quiet.
  import { onMount } from "svelte";
  import Icon from "../../lib/Icon.svelte";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { monitor } from "../../lib/stores/monitor.svelte";
  import type { AlertRule } from "../../lib/types/generated/AlertRule";
  import type { Comparison } from "../../lib/types/generated/Comparison";
  import type { Metric } from "../../lib/types/generated/Metric";
  import { METRICS, METRIC_ORDER, clampThreshold } from "../island/monitor/metrics";

  /** How long a condition must hold, in seconds. */
  const DURATIONS = [0, 10, 30, 60, 120, 300, 600];

  /** No temperature sensor visible (common on Windows without admin rights). */
  const noSensor = $derived(monitor.latest !== undefined && monitor.latest.temperature === null);

  onMount(() => {
    let off: (() => void) | undefined;
    void monitor.connect().then((unlisten) => (off = unlisten));
    return () => off?.();
  });

  async function save(rule: AlertRule, change: Partial<AlertRule>) {
    const next = { ...rule, ...change };
    // Switching metric starts from that metric's sensible default (units differ).
    if (change.metric && change.metric !== rule.metric) {
      next.threshold = METRICS[change.metric].threshold;
      next.op = METRICS[change.metric].op;
    }
    next.threshold = clampThreshold(next.metric, next.threshold);
    next.durationS = Math.min(3600, Math.max(0, Math.round(next.durationS)));
    const result = await monitor.run(ipc.alertRuleSave(next));
    if (result.ok) monitor.rules = result.value;
  }

  async function remove(id: string) {
    const result = await monitor.run(ipc.alertRuleDelete(id));
    if (result.ok) monitor.rules = result.value;
  }

  function add() {
    void save(
      { id: "", metric: "cpu", op: "above", threshold: 90, durationS: 60, enabled: true },
      {},
    );
  }

  function durations(current: number): number[] {
    return DURATIONS.includes(current) ? DURATIONS : [...DURATIONS, current].sort((a, b) => a - b);
  }
</script>

<div class="system">
  <h3>{t.monitor.alertsTitle}</h3>
  <p class="muted">{t.monitor.alertsIntro}</p>

  <ul>
    {#each monitor.rules as rule (rule.id)}
      <li class:off={!rule.enabled}>
        <input
          type="checkbox"
          role="switch"
          checked={rule.enabled}
          onchange={(e) => save(rule, { enabled: e.currentTarget.checked })}
          aria-label={t.monitor.enabled}
        />
        <select
          class="field metric"
          value={rule.metric}
          onchange={(e) => save(rule, { metric: e.currentTarget.value as Metric })}
          aria-label={t.monitor.what}
        >
          {#each METRIC_ORDER as metric (metric)}
            <option value={metric}>{t.monitor.metric[metric]}</option>
          {/each}
        </select>
        <select
          class="field"
          value={rule.op}
          onchange={(e) => save(rule, { op: e.currentTarget.value as Comparison })}
          aria-label={t.monitor.direction}
        >
          <option value="above">{t.monitor.above}</option>
          <option value="below">{t.monitor.below}</option>
        </select>
        <span class="limit">
          <input
            class="field num"
            type="number"
            min="0"
            max={METRICS[rule.metric].max}
            value={rule.threshold}
            onchange={(e) => save(rule, { threshold: Number(e.currentTarget.value) })}
            aria-label={t.monitor.threshold}
          />
          <span class="unit">{METRICS[rule.metric].unit}</span>
        </span>
        <select
          class="field"
          value={rule.durationS}
          onchange={(e) => save(rule, { durationS: Number(e.currentTarget.value) })}
          aria-label={t.monitor.holdsFor}
        >
          {#each durations(rule.durationS) as seconds (seconds)}
            <option value={seconds}>{t.monitor.durationOption(seconds)}</option>
          {/each}
        </select>
        <button class="icon" onclick={() => remove(rule.id)} aria-label={t.monitor.deleteRule}>
          <Icon name="trash" size={14} />
        </button>
        {#if rule.metric === "temperature" && noSensor}
          <small class="warn">{t.monitor.noSensor}</small>
        {/if}
      </li>
    {:else}
      <li class="muted">{t.monitor.noRules}</li>
    {/each}
  </ul>

  <button class="ghost" onclick={add}><Icon name="plus" size={14} />{t.monitor.addRule}</button>
  <label class="repeat">
    <span>{t.monitor.repeat}</span>
    <select
      class="field"
      value={monitor.alertRepeatMin}
      onchange={async (e) => {
        const result = await monitor.run(ipc.alertRepeatSet(Number(e.currentTarget.value)));
        if (result.ok) monitor.alertRepeatMin = result.value;
      }}
    >
      <option value={0}>{t.monitor.repeatOnce}</option>
      {#each [15, 30, 60, 120] as minutes (minutes)}
        <option value={minutes}>{t.monitor.repeatEvery(minutes)}</option>
      {/each}
    </select>
  </label>
  <p class="muted">{t.monitor.quietNote}</p>
  {#if monitor.error}
    <p class="error" role="alert">{monitor.error}</p>
  {/if}
</div>

<style>
  .system {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  h3 {
    margin: 0;
    font: 700 15px / 20px var(--font-ui);
  }

  .muted {
    margin: 0;
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: var(--space-1) 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: var(--space-2);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  li.off > :not(input[type="checkbox"]) {
    opacity: 0.55;
  }

  .field {
    padding: 6px var(--space-2);
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .metric {
    flex: 1;
    min-width: 96px;
  }

  select.field {
    padding-right: 2px;
  }

  .limit {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .num {
    width: 60px;
    text-align: right;
  }

  .unit {
    min-width: 16px;
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
  }

  .icon:hover {
    color: var(--danger);
    background: var(--surface);
  }

  .warn {
    flex-basis: 100%;
    font: 400 12px / 16px var(--font-ui);
    color: var(--warning);
  }

  .ghost {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px var(--space-3);
    font: var(--text-pill);
    color: var(--brand);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .repeat {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin-top: var(--space-2);
    font: 500 14px / 20px var(--font-ui);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font: 500 13px / 18px var(--font-ui);
  }

  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 36px;
    height: 20px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--surface);
    border: 1px solid var(--border);
    cursor: pointer;
  }

  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--mod-monitor);
    border-color: var(--mod-monitor);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    background: #fff;
    transform: translateX(16px);
  }
</style>
