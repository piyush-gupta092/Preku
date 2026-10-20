<script lang="ts">
  // Pip in a role, used in place of a module's line icon: Focus Pip (productivity), Tech Pip
  // (system monitor) and Zen Pip (wellbeing). A static SVG in the user's Pip color, so three
  // of them on screen cost nothing (the animated canvas Pip stays one per view, Rules.md §5).
  import { paletteFor } from "../pip/palette";
  import { mascot } from "./stores/mascot.svelte";

  export type MascotModule = "productivity" | "monitor" | "wellbeing";

  interface Props {
    module: MascotModule;
    size?: number;
  }
  let { module, size = 32 }: Props = $props();

  mascot.ensureConnected();
  const palette = $derived(paletteFor(mascot.color));
</script>

<svg
  class="module-mascot"
  viewBox="0 0 64 64"
  width={size}
  height={size}
  aria-hidden="true"
  style:--body={palette.body}
  style:--shade={palette.shade}
  style:--outline={palette.outline}
  style:--cheek={palette.cheek}
  style:--eye={palette.eye}
  style:--leaf={palette.leaf}
>
  <!-- Pip: two leaves and a round body. -->
  <path class="leaf" d="M31 17 C 27 7, 16 5, 11 10 C 15 17, 24 19, 31 17 Z" />
  <path class="leaf" d="M33 17 C 37 7, 48 5, 53 10 C 49 17, 40 19, 33 17 Z" />
  <ellipse class="body" cx="32" cy="38" rx="23" ry="20" />
  <path class="shade" d="M12 46 C 20 58, 44 58, 52 46 C 46 53, 18 53, 12 46 Z" />

  {#if module === "productivity"}
    <!-- Focus Pip: headband, determined eyes, stopwatch. -->
    <path class="band" d="M10.5 31 C 20 25, 44 25, 53.5 31" />
    <path class="band tail" d="M53 30 L 60 25 M53 31 L 61 32" />
    <path class="brow" d="M19 33 L 27 35.5 M45 33 L 37 35.5" />
    <ellipse class="eye" cx="24" cy="40" rx="3" ry="3.6" />
    <ellipse class="eye" cx="40" cy="40" rx="3" ry="3.6" />
    <path class="mouth" d="M28 47.5 Q 32 50 36 47.5" />
    <g class="prop">
      <rect class="accent-fill" x="47.4" y="37.5" width="4.2" height="4" rx="1" />
      <circle class="face" cx="49.5" cy="50" r="9" />
      <path class="hand" d="M49.5 50 L 49.5 44.5 M49.5 50 L 53.5 52" />
    </g>
  {:else if module === "monitor"}
    <!-- Tech Pip: round glasses and a little screen with a pulse. -->
    <ellipse class="eye" cx="23" cy="38" rx="2.4" ry="3" />
    <ellipse class="eye" cx="41" cy="38" rx="2.4" ry="3" />
    <circle class="glass" cx="23" cy="38" r="7" />
    <circle class="glass" cx="41" cy="38" r="7" />
    <path class="glass" d="M30 38 L 34 38" />
    <path class="mouth" d="M29 49 Q 32 51 35 49" />
    <g class="prop">
      <rect class="screen" x="38" y="43" width="23" height="15" rx="3" />
      <path class="pulse" d="M41 51 L 45 51 L 47.5 46.5 L 51 55 L 53.5 51 L 58 51" />
      <path class="stand" d="M46 61 L 53 61" />
    </g>
  {:else}
    <!-- Zen Pip: happy closed eyes, rosy cheeks, a drop of water. -->
    <path class="closed" d="M19 39 Q 23.5 34.5 28 39 M36 39 Q 40.5 34.5 45 39" />
    <ellipse class="cheek" cx="17" cy="44" rx="4" ry="2.8" />
    <ellipse class="cheek" cx="47" cy="44" rx="4" ry="2.8" />
    <path class="mouth" d="M27 45.5 Q 32 50.5 37 45.5" />
    <g class="prop">
      <path
        class="drop"
        d="M51 39 C 46 46, 44 50, 44 53 A 7 7 0 0 0 58 53 C 58 50, 56 46, 51 39 Z"
      />
      <path class="shine" d="M48 53 A 3 3 0 0 0 51 56" />
    </g>
  {/if}
</svg>

<style>
  .module-mascot {
    display: block;
    overflow: visible;
  }

  .leaf {
    fill: var(--leaf);
    stroke: var(--outline);
    stroke-width: 1.2;
    stroke-linejoin: round;
  }

  .body {
    fill: var(--body);
    stroke: var(--outline);
    stroke-width: 1.6;
  }

  .shade {
    fill: var(--shade);
    opacity: 0.7;
  }

  .eye {
    fill: var(--eye);
  }

  .cheek {
    fill: var(--cheek);
    opacity: 0.75;
  }

  .mouth,
  .closed,
  .brow {
    fill: none;
    stroke: var(--eye);
    stroke-width: 2.2;
    stroke-linecap: round;
  }

  /* ---- Focus Pip ---- */
  .band {
    fill: none;
    stroke: var(--accent, #ffb020);
    stroke-width: 4;
    stroke-linecap: round;
  }

  .band.tail {
    stroke-width: 3;
  }

  .accent-fill {
    fill: var(--accent, #ffb020);
    stroke: var(--outline);
    stroke-width: 1;
  }

  .face {
    fill: #fffaf2;
    stroke: var(--accent, #ffb020);
    stroke-width: 3;
  }

  .hand {
    fill: none;
    stroke: #2a2233;
    stroke-width: 2;
    stroke-linecap: round;
  }

  /* ---- Tech Pip ---- */
  .glass {
    fill: rgba(255, 255, 255, 0.28);
    stroke: #2a2233;
    stroke-width: 2;
  }

  .screen {
    fill: #14141a;
    stroke: var(--accent, #22c7b8);
    stroke-width: 2;
  }

  .pulse {
    fill: none;
    stroke: var(--accent, #22c7b8);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .stand {
    stroke: var(--accent, #22c7b8);
    stroke-width: 2.4;
    stroke-linecap: round;
  }

  /* ---- Zen Pip ---- */
  .drop {
    fill: #6ec8ff;
    stroke: #2f7fb8;
    stroke-width: 1.4;
    stroke-linejoin: round;
  }

  .shine {
    fill: none;
    stroke: #fff;
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>
