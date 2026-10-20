<script lang="ts">
  // Dev-only: every mood and every dance move side by side, for tuning Pip
  // (preview.html?window=gallery&size=96).
  import Pip from "../pip/Pip.svelte";
  import { DANCE_MOVES, danceFrame, type DanceMove } from "../pip/dance";
  import { EXPRESSIONS } from "../pip/expressions";
  import type { MascotStyle } from "../lib/types/generated/MascotStyle";
  import { DEFAULT_MASCOT_COLOR, paletteFor } from "../pip/palette";
  import { drawPip, type PipMood } from "../pip/renderer";

  const moods = (Object.keys(EXPRESSIONS) as PipMood[]).filter((mood) => mood !== "dance");
  const size = Number(new URLSearchParams(location.search).get("size") ?? 96);

  // Optional ?style=classic|sprout|ghost|chubby and ?color=%23rrggbb.
  const params = new URLSearchParams(location.search);
  const style = (params.get("style") ?? "classic") as MascotStyle;
  const color = params.get("color") ?? DEFAULT_MASCOT_COLOR;
  const palette = () => paletteFor(color);

  /** Draws one frame: a dance move, or the top of a celebrate hop (the clipping check). */
  function still(canvas: HTMLCanvasElement, what: { move?: DanceMove; time: number }) {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvas.height = size * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawPip(
      ctx,
      size,
      {
        mood: what.move ? "dance" : "celebrate",
        breathe: 1,
        squish: 1,
        hop: what.move ? 0 : 1,
        eyeOpen: 1,
        lookX: 0,
        lookY: 0,
        time: what.time,
        dance: what.move ? danceFrame(what.move, what.time) : undefined,
      },
      palette(),
      style,
    );
  }
</script>

<main>
  {#each moods as mood (mood)}
    <figure>
      <Pip {size} {mood} {style} {color} label={mood} />
      <figcaption>{mood}</figcaption>
    </figure>
  {/each}
  <figure>
    <canvas use:still={{ time: 0 }} style:width="{size}px" style:height="{size}px"></canvas>
    <figcaption>hop (top)</figcaption>
  </figure>
  <figure class="small">
    <Pip size={24} mood="idle" {style} {color} label="pill size" />
    <figcaption>24 px</figcaption>
  </figure>
</main>

<main>
  {#each DANCE_MOVES as move (move)}
    <figure>
      <canvas use:still={{ move, time: 1375 }} style:width="{size}px" style:height="{size}px"
      ></canvas>
      <figcaption>{move}</figcaption>
    </figure>
  {/each}
</main>

<style>
  :global(body) {
    background: var(--island-bg);
  }

  main {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    padding: var(--space-4);
  }

  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
  }

  canvas {
    outline: 1px dashed color-mix(in srgb, var(--island-text-muted) 30%, transparent);
  }

  figcaption {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .small {
    justify-content: flex-end;
  }
</style>
