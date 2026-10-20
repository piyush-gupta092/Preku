<script lang="ts">
  // Alerts in the notch: what needs attention right now, and which alerts are on. Reading and
  // switching only; thresholds and durations are edited on the roomier System page in
  // Settings (the rule editor didn't fit here).
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { monitor } from "../../../lib/stores/monitor.svelte";
  import type { AlertRule } from "../../../lib/types/generated/AlertRule";
  import type { LiveItem } from "../../../lib/types/generated/LiveItem";
  import { ruleSentence } from "./metrics";

  interface Props {
    /** Live items in the notch; alerts that are firing are among them. */
    items: LiveItem[];
    /** Opens Settings → Modules → System. */
    onOpenPage: () => void;
  }
  let { items, onOpenPage }: Props = $props();

  const firing = $derived(items.filter((item) => item.id.startsWith("monitor.alert.")));
  const firingIds = $derived(new Set(firing.map((item) => item.id.slice("monitor.alert.".length))));

  async function toggle(rule: AlertRule, enabled: boolean) {
    const result = await monitor.run(ipc.alertRuleSave({ ...rule, enabled }));
    if (result.ok) monitor.rules = result.value;
  }

  function sentence(rule: AlertRule): string {
    return ruleSentence(rule, {
      metric: t.monitor.metric[rule.metric] ?? rule.metric,
      above: t.monitor.above,
      below: t.monitor.below,
      forDuration: t.monitor.forDuration,
    });
  }
</script>

<div class="alerts">
  <div class="now" class:calm={firing.length === 0} role="status">
    {#if firing.length === 0}
      <span class="dot ok" aria-hidden="true"></span>
      <span>{t.monitor.allClear}</span>
    {:else}
      <ul>
        {#each firing as item (item.id)}
          <li>
            <span class="dot" aria-hidden="true"></span>
            <span class="what">{item.title}</span>
            {#if item.subtitle}<span class="detail">{item.subtitle}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <ul class="rules pk-scroll">
    {#each monitor.rules as rule (rule.id)}
      <li class:off={!rule.enabled} class:firing={firingIds.has(rule.id)}>
        <input
          type="checkbox"
          role="switch"
          checked={rule.enabled}
          onchange={(e) => toggle(rule, e.currentTarget.checked)}
          aria-label={sentence(rule)}
        />
        <span class="sentence">{sentence(rule)}</span>
      </li>
    {:else}
      <li class="pk-muted">{t.monitor.noRules}</li>
    {/each}
  </ul>

  <button class="edit" onclick={onOpenPage}>{t.monitor.editAlerts} ›</button>
</div>

<style>
  .alerts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  .now {
    padding: var(--space-2) var(--space-3);
    font: var(--text-body);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    border-radius: var(--radius-md);
  }

  .now.calm {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--island-text-muted);
    background: var(--island-bg-raised);
    border-color: var(--island-border);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .now li {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-height: 24px;
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }

  .dot.ok {
    background: var(--success);
  }

  .what {
    font-weight: 600;
  }

  .detail {
    font: var(--text-mono);
    color: var(--island-text-muted);
  }

  .rules {
    flex: 1;
    min-height: 0;
  }

  .rules li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 34px;
    padding: 0 var(--space-1);
    border-radius: var(--radius-sm);
  }

  .rules li.off .sentence {
    color: var(--island-text-muted);
  }

  .rules li.firing .sentence {
    color: var(--danger);
  }

  .sentence {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .edit {
    align-self: flex-start;
    padding: var(--space-1) var(--space-2);
    font: var(--text-pill);
    color: var(--island-text-muted);
  }

  .edit:hover {
    color: var(--island-text);
  }

  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 30px;
    height: 18px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--island-bg);
    border: 1px solid var(--island-border);
    cursor: pointer;
  }

  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--island-text-muted);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    background: var(--island-bg);
    transform: translateX(12px);
  }
</style>
