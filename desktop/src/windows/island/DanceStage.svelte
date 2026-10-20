<script lang="ts">
  // The dance floor: replaces the module tiles while the user watches Pip dance to the music
  // (click the now-playing card to open/close). Pip is drawn big; the move name follows the
  // same clock as the dance itself.
  import { onMount } from "svelte";
  import Icon from "../../lib/Icon.svelte";
  import { t } from "../../lib/i18n";
  import { currentMove, type DanceMove } from "../../pip/dance";
  import Pip from "../../pip/Pip.svelte";

  interface Props {
    playing: boolean;
    onclose: () => void;
  }

  let { playing, onclose }: Props = $props();

  let move = $state<DanceMove>(currentMove(performance.now()));
  let pip: Pip | undefined = $state();

  onMount(() => {
    const timer = setInterval(() => (move = currentMove(performance.now())), 250);
    return () => clearInterval(timer);
  });
</script>

<div class="stage" class:playing aria-label={t.dance.stage}>
  <div class="lights" aria-hidden="true">
    <span></span><span></span><span></span>
  </div>
  <div class="spot" aria-hidden="true"></div>
  <div class="floor" aria-hidden="true"></div>

  <button class="dancer" onclick={() => pip?.poke()} aria-label={t.petPip}>
    <Pip bind:this={pip} size={160} mood={playing ? "dance" : "idle"} label={t.pipLabel} />
  </button>

  <span class="move" aria-live="polite">
    {#if playing}
      <Icon name="music" size={12} />
      {t.dance.moves[move]}
    {:else}
      {t.dance.paused}
    {/if}
  </span>
  <button class="close" onclick={onclose} aria-label={t.dance.back} title={t.dance.back}>
    <Icon name="close" size={14} />
  </button>
</div>

<style>
  .stage {
    position: relative;
    height: 156px;
    overflow: hidden;
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
    background:
      radial-gradient(
        120% 90% at 50% 0%,
        color-mix(in srgb, var(--brand) 22%, transparent),
        transparent 70%
      ),
      linear-gradient(180deg, #17142a, #0d0c14);
  }

  /* A cone of light from the top centre. */
  .spot {
    position: absolute;
    inset: 0;
    background: conic-gradient(
      from 180deg at 50% -10%,
      transparent 160deg,
      rgba(255, 255, 255, 0.1) 172deg,
      rgba(255, 255, 255, 0.16) 180deg,
      rgba(255, 255, 255, 0.1) 188deg,
      transparent 200deg
    );
    pointer-events: none;
  }

  .floor {
    position: absolute;
    left: 50%;
    bottom: -26px;
    width: 300px;
    height: 56px;
    transform: translateX(-50%);
    border-radius: 50%;
    background:
      radial-gradient(closest-side, rgba(255, 255, 255, 0.16), transparent),
      repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.05) 0 24px, transparent 24px 48px);
    pointer-events: none;
  }

  /* Disco lights that pulse on the beat (120 BPM) while the music plays. */
  .lights span {
    position: absolute;
    width: 90px;
    height: 90px;
    border-radius: 50%;
    filter: blur(28px);
    opacity: 0.25;
  }

  .lights span:nth-child(1) {
    top: -20px;
    left: 8%;
    background: var(--mod-productivity);
  }

  .lights span:nth-child(2) {
    top: 10px;
    right: 6%;
    background: var(--brand);
  }

  .lights span:nth-child(3) {
    bottom: -30px;
    left: 40%;
    background: var(--mod-monitor);
  }

  .playing .lights span {
    animation: pulse 500ms ease-in-out infinite alternate;
  }

  .playing .lights span:nth-child(2) {
    animation-delay: 250ms;
  }

  .playing .lights span:nth-child(3) {
    animation-duration: 1000ms;
  }

  @keyframes pulse {
    from {
      opacity: 0.15;
      transform: scale(0.9);
    }
    to {
      opacity: 0.45;
      transform: scale(1.15);
    }
  }

  .dancer {
    position: absolute;
    left: 50%;
    bottom: 2px;
    transform: translateX(-50%);
    border-radius: var(--radius-lg);
  }

  .move {
    position: absolute;
    left: var(--space-3);
    bottom: var(--space-2);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 2px var(--space-2);
    font: var(--text-pill);
    color: var(--island-text);
    background: rgba(0, 0, 0, 0.35);
    border-radius: var(--radius-pill);
  }

  .close {
    position: absolute;
    top: var(--space-2);
    right: var(--space-2);
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    color: var(--island-text-muted);
    background: rgba(0, 0, 0, 0.3);
    border-radius: 50%;
  }

  .close:hover {
    color: var(--island-text);
    background: rgba(0, 0, 0, 0.5);
  }

  @media (prefers-reduced-motion: reduce) {
    .playing .lights span {
      animation: none;
    }
  }
</style>
