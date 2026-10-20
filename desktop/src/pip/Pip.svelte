<script lang="ts">
  import { onMount } from "svelte";
  import { mascot } from "../lib/stores/mascot.svelte";
  import type { MascotStyle } from "../lib/types/generated/MascotStyle";
  import { PipAnimator } from "./animator";
  import { paletteFor } from "./palette";
  import { drawPip, type PipMood } from "./renderer";

  interface Props {
    size?: number;
    mood?: PipMood;
    /** Frame cap: 60 while interacting, lower when idle to save CPU (Rules.md §5). */
    fps?: number;
    /** Stop the loop entirely (e.g. island hidden). */
    paused?: boolean;
    /** Override the user's chosen body/color (Settings previews). */
    style?: MascotStyle;
    color?: string;
    label: string;
  }

  let { size = 24, mood = "idle", fps = 60, paused = false, style, color, label }: Props = $props();

  mascot.ensureConnected();
  const bodyStyle = $derived(style ?? mascot.style);
  const palette = $derived(paletteFor(color ?? mascot.color));

  let canvas: HTMLCanvasElement;
  const animator = new PipAnimator(performance.now());
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

  /** Squish, and get dizzy if poked too often. */
  export function poke(): void {
    animator.poke(performance.now());
  }

  function draw(now: number) {
    const ctx = canvas?.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const px = Math.round(size * dpr);
    if (canvas.width !== px) {
      canvas.width = px;
      canvas.height = px;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawPip(ctx, size, animator.pose(now, mood, reducedMotion.matches), palette, bodyStyle);
  }

  onMount(() => {
    // Eyes follow the cursor while it is over the island.
    const onMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const range = Math.max(window.innerWidth, 200) / 2;
      animator.look(
        (e.clientX - (rect.left + rect.width / 2)) / range,
        (e.clientY - (rect.top + rect.height / 2)) / range,
      );
    };
    window.addEventListener("mousemove", onMove);
    return () => window.removeEventListener("mousemove", onMove);
  });

  $effect(() => {
    // Re-runs when size, mood, look, fps or paused change.
    void size;
    void mood;
    void bodyStyle;
    void palette;
    if (paused) {
      draw(performance.now());
      return;
    }
    const frameMs = 1000 / fps;
    let last = 0;
    let raf = requestAnimationFrame(function frame(now) {
      raf = requestAnimationFrame(frame);
      if (now - last < frameMs) return;
      last = now;
      draw(now);
    });
    return () => cancelAnimationFrame(raf);
  });
</script>

<span class="pip" role="img" aria-label={label}>
  <canvas bind:this={canvas} style:width="{size}px" style:height="{size}px" aria-hidden="true"
  ></canvas>
</span>

<style>
  .pip,
  canvas {
    display: block;
    flex: none;
  }
</style>
