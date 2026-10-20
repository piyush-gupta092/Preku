<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { formatBytes, formatPercent } from "../../../lib/format";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { monitor } from "../../../lib/stores/monitor.svelte";
  import type { ProcessInfo } from "../../../lib/types/generated/ProcessInfo";

  /** Ending a process always asks first (Rules.md §4). */
  let confirming = $state<ProcessInfo | null>(null);
  /** Bars are relative to the busiest process, so small differences stay visible. */
  const busiest = $derived(Math.max(1, ...monitor.processes.map((p) => p.cpu)));

  async function end(process: ProcessInfo) {
    confirming = null;
    const result = await monitor.run(ipc.processKill(process.pid));
    if (result.ok) monitor.processes = result.value;
  }
</script>

<div class="processes">
  {#if confirming}
    <div class="confirm" role="alertdialog" aria-label={t.monitor.endProcess(confirming.name)}>
      <span class="pk-grow">{t.monitor.confirmEnd(confirming.name)}</span>
      <button class="pk-button danger" onclick={() => confirming && end(confirming)}>
        {t.monitor.end}
      </button>
      <button class="pk-button quiet" onclick={() => (confirming = null)}>{t.monitor.cancel}</button
      >
    </div>
  {/if}

  <div class="head pk-muted">
    <span class="pk-grow">{t.monitor.processName}</span>
    <span class="num">{t.monitor.processCpu}</span>
    <span class="num">{t.monitor.processMemory}</span>
    <span class="spacer"></span>
  </div>

  <ul class="pk-scroll">
    {#each monitor.processes as process (process.pid)}
      <li class="pk-row">
        <span class="pk-grow" title="PID {process.pid}">{process.name}</span>
        <span class="num cpu">
          <span class="meter" style:width="{(process.cpu / busiest) * 100}%"></span>
          {formatPercent(process.cpu)}
        </span>
        <span class="num">{formatBytes(process.memory)}</span>
        <button
          class="pk-icon-button"
          onclick={() => (confirming = process)}
          aria-label={t.monitor.endProcess(process.name)}
        >
          <Icon name="close" size={14} />
        </button>
      </li>
    {:else}
      <li class="pk-muted empty">{t.monitor.loadingProcesses}</li>
    {/each}
  </ul>
</div>

<style>
  .processes {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    height: 100%;
    min-height: 0;
  }

  ul {
    flex: 1;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .head {
    display: flex;
    gap: var(--space-2);
    padding: 0 var(--space-2);
  }

  .num {
    flex: none;
    width: 72px;
    text-align: right;
    font: var(--text-mono);
  }

  .head .num {
    font: var(--text-caption);
  }

  .spacer {
    width: 26px;
  }

  .cpu {
    position: relative;
    isolation: isolate;
  }

  .meter {
    position: absolute;
    inset: 3px auto 3px 0;
    max-width: 100%;
    border-radius: 3px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    z-index: -1;
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: color-mix(in srgb, var(--danger) 12%, var(--island-bg-raised));
    border: 1px solid color-mix(in srgb, var(--danger) 40%, transparent);
    border-radius: var(--radius-sm);
    font: var(--text-caption);
  }

  .empty {
    padding: var(--space-6);
    text-align: center;
  }
</style>
