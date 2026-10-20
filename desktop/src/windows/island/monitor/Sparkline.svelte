<script lang="ts">
  // Tiny line chart for a metric's recent history. Fixed 0…max scale so a flat line at 5%
  // doesn't look dramatic; `max` defaults to the data's own peak (e.g. network rates).
  interface Props {
    values: number[];
    max?: number;
    height?: number;
  }

  let { values, max, height = 28 }: Props = $props();

  const width = 100;
  const points = $derived.by(() => {
    if (values.length < 2) return "";
    const top = max ?? Math.max(...values, 1);
    const step = width / (values.length - 1);
    return values
      .map((v, i) => {
        const y = height - (Math.min(Math.max(v, 0), top) / top) * (height - 2) - 1;
        return `${(i * step).toFixed(2)},${y.toFixed(2)}`;
      })
      .join(" ");
  });
</script>

<svg
  viewBox="0 0 {width} {height}"
  preserveAspectRatio="none"
  style:height="{height}px"
  aria-hidden="true"
>
  {#if points}
    <polygon class="area" points="0,{height} {points} {width},{height}" />
    <polyline class="line" {points} />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
  }

  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }

  .area {
    fill: color-mix(in srgb, var(--accent) 16%, transparent);
  }
</style>
