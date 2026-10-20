<script lang="ts">
  // Text that scrolls in a loop when it doesn't fit (e.g. long song titles), and sits still
  // when it does. Reduced motion: no scrolling; the full text is in the tooltip.
  import { onMount } from "svelte";

  interface Props {
    text: string;
    /** Scroll speed in px per second. */
    speed?: number;
    /** Space between the end of the text and its repeat, in px. */
    gap?: number;
  }

  let { text, speed = 30, gap = 32 }: Props = $props();

  let box: HTMLSpanElement;
  let track: HTMLSpanElement;
  let measure: HTMLSpanElement;
  let overflow = $state(false);
  let distance = $state(0);
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");

  function check() {
    if (!box || !track || !measure) return;
    // Inline text reports no scroll width of its own: while still, measure the truncating
    // block around it; while moving, the first copy (a flex item, so it has a real box).
    const textWidth = overflow ? measure.offsetWidth : track.scrollWidth;
    overflow = !reduced.matches && textWidth > box.clientWidth + 1;
    distance = textWidth + gap;
  }

  onMount(() => {
    const observer = new ResizeObserver(check);
    observer.observe(box);
    observer.observe(track);
    return () => observer.disconnect();
  });

  // Re-measure when the text changes (next track).
  $effect(() => {
    void text;
    queueMicrotask(check);
  });
</script>

<span class="marquee" bind:this={box} title={text}>
  <span
    bind:this={track}
    class="track"
    class:moving={overflow}
    style:--distance="{distance}px"
    style:--duration="{distance / speed}s"
    style:--gap="{gap}px"
  >
    <span class="item" bind:this={measure}>{text}</span>
    {#if overflow}<span class="item" aria-hidden="true">{text}</span>{/if}
  </span>
</span>

<style>
  .marquee {
    display: block;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }

  /* Soft fade at both edges while scrolling, so text doesn't cut off abruptly. */
  .marquee:has(.moving) {
    mask-image: linear-gradient(90deg, transparent, #000 8px, #000 calc(100% - 12px), transparent);
  }

  .track {
    display: inline-flex;
    gap: var(--gap);
  }

  .item {
    flex: none;
  }

  /* When it fits (or with reduced motion) the single copy just truncates normally. */
  .track:not(.moving) {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .moving {
    animation: scroll var(--duration) linear infinite;
    /* A short pause at the start of each loop so the beginning is readable. */
    animation-delay: 1.5s;
  }

  .marquee:hover .moving {
    animation-play-state: paused;
  }

  @keyframes scroll {
    to {
      transform: translateX(calc(-1 * var(--distance)));
    }
  }
</style>
