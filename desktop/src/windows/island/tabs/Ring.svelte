<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    size: number;
    /** 0…1 */
    value: number;
    stroke?: number;
    children?: Snippet;
  }

  let { size, value, stroke = 6, children }: Props = $props();
  const r = $derived((size - stroke) / 2);
</script>

<div class="ring" style:width="{size}px" style:height="{size}px">
  <svg viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle class="track" cx={size / 2} cy={size / 2} {r} stroke-width={stroke} />
    <!-- A zero-length round-capped stroke would still draw a dot. -->
    <circle
      class="value"
      visibility={value > 0 ? "visible" : "hidden"}
      cx={size / 2}
      cy={size / 2}
      {r}
      stroke-width={stroke}
      pathLength="100"
      stroke-dasharray="{Math.min(1, Math.max(0, value)) * 100} 100"
    />
  </svg>
  {#if children}<div class="center">{@render children()}</div>{/if}
</div>

<style>
  .ring {
    position: relative;
    flex: none;
  }

  svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
  }

  .track {
    stroke: var(--island-bg-raised);
  }

  .value {
    stroke: var(--accent, var(--mod-productivity));
    stroke-linecap: round;
    transition: stroke-dasharray var(--dur-slow) var(--ease-out);
  }

  .center {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    text-align: center;
  }
</style>
