<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import {
    formatBytes,
    formatDuration,
    formatPercent,
    formatRate,
    percentOf,
  } from "../../../lib/format";
  import { t } from "../../../lib/i18n";
  import { monitor } from "../../../lib/stores/monitor.svelte";
  import Sparkline from "./Sparkline.svelte";

  const latest = $derived(monitor.latest);
  /** Charts show the last minute or so ("now"), or the last hour as one point per minute. */
  let range = $state<"now" | "hour">("now");
  const history = $derived(range === "hour" ? monitor.historyHour : monitor.history);
  const memPct = $derived(latest ? percentOf(latest.memUsed, latest.memTotal) : 0);
  const diskPct = $derived(latest ? percentOf(latest.diskUsed, latest.diskTotal) : 0);
</script>

{#if latest}
  <div class="overview">
    <div class="range" role="group" aria-label={t.monitor.lastHour}>
      <button
        class:on={range === "now"}
        aria-pressed={range === "now"}
        onclick={() => (range = "now")}
      >
        {t.monitor.now}
      </button>
      <button
        class:on={range === "hour"}
        aria-pressed={range === "hour"}
        onclick={() => (range = "hour")}
      >
        {t.monitor.lastHour}
      </button>
      {#if range === "hour" && monitor.historyHour.length < 2}
        <span class="pk-muted">{t.monitor.hourEmpty}</span>
      {/if}
    </div>
    <div class="grid">
      <section class="card">
        <header><Icon name="gauge" size={14} />{t.monitor.cpu}</header>
        <span class="big">{formatPercent(latest.cpu)}</span>
        <Sparkline values={history.map((s) => s.cpu)} max={100} />
      </section>

      <section class="card">
        <header><Icon name="box" size={14} />{t.monitor.memory}</header>
        <span class="big">{formatPercent(memPct)}</span>
        <span class="pk-muted">
          {t.monitor.ofTotal(formatBytes(latest.memUsed), formatBytes(latest.memTotal))}
        </span>
        <Sparkline
          values={history.map((s) => percentOf(s.memUsed, s.memTotal))}
          max={100}
          height={18}
        />
      </section>

      <section class="card">
        <header>
          <Icon name="list" size={14} />{t.monitor.disk} <span class="dim">{latest.diskName}</span>
        </header>
        <span class="big">{formatPercent(diskPct)}</span>
        <span class="pk-muted"
          >{t.monitor.free(formatBytes(latest.diskTotal - latest.diskUsed))}</span
        >
        <div class="bar" class:warn={diskPct >= 90}>
          <span style:width="{Math.min(100, diskPct)}%"></span>
        </div>
      </section>

      <section class="card">
        <header><Icon name="globe" size={14} />{t.monitor.network}</header>
        <span class="rates">
          <span>↓ {formatRate(latest.netRx)}</span>
          <span>↑ {formatRate(latest.netTx)}</span>
        </span>
        <Sparkline values={history.map((s) => s.netRx + s.netTx)} />
      </section>
    </div>

    <p class="battery pk-muted">
      {#if latest.battery}
        <span>
          🔋 {t.monitor.battery}
          {latest.battery.percent}%{latest.battery.charging ? ` · ${t.monitor.charging}` : ""}
          {#if latest.battery.secsLeft}
            · {t.monitor.timeLeft(formatDuration(latest.battery.secsLeft))}
          {/if}
        </span>
      {/if}
      <span>
        🌡 {t.monitor.temperature}:
        {typeof latest.temperature === "number"
          ? `${Math.round(latest.temperature)} °C`
          : t.monitor.noSensorShort}
      </span>
    </p>
  </div>
{:else}
  <p class="pk-muted waiting">{t.monitor.tileIdle}</p>
{/if}

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-auto-rows: minmax(0, 1fr);
    gap: var(--space-2);
  }

  .card {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 2px;
    padding: var(--space-2) var(--space-3);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: 6px;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .dim {
    margin-left: auto;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .big {
    font: var(--text-title);
    font-family: var(--font-mono);
  }

  .rates {
    display: flex;
    flex-direction: column;
    font: var(--text-mono);
  }

  .bar {
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--island-bg);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
  }

  .bar.warn span {
    background: var(--danger);
  }

  .battery {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--space-4);
    margin: 0;
  }

  .range {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .range button {
    padding: 2px 10px;
    font: var(--text-pill);
    color: var(--island-text-muted);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .range button.on {
    color: var(--island-bg);
    background: var(--accent);
    border-color: var(--accent);
  }

  .waiting {
    display: grid;
    place-items: center;
    height: 100%;
    margin: 0;
  }
</style>
