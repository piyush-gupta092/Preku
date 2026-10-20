<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import Marquee from "../../../lib/Marquee.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { productivity } from "../../../lib/stores/productivity.svelte";
  import type { MediaAction } from "../../../lib/types/generated/MediaAction";

  interface Props {
    /** The dance stage is showing (the card opens/closes it). */
    staged?: boolean;
    onstage?: () => void;
  }

  let { staged = false, onstage }: Props = $props();

  const track = $derived(productivity.media);

  async function send(action: MediaAction) {
    const result = await productivity.run(ipc.mediaCommand(action));
    if (result.ok) productivity.media = result.value;
  }
</script>

{#if track}
  <div class="card" class:staged>
    <button
      class="stage-toggle"
      onclick={() => onstage?.()}
      aria-pressed={staged}
      title={staged ? t.dance.back : t.dance.open}
    >
      <span class="art" aria-hidden="true">
        <Icon name={staged ? "close" : "music"} size={staged ? 14 : 18} />
      </span>
      <span class="text">
        <span class="title"><Marquee text={track.title} /></span>
        <span class="sub">
          {staged ? t.dance.back : [track.artist, track.app].filter(Boolean).join(" · ")}
        </span>
      </span>
    </button>
    <span class="controls">
      <button
        onclick={() => send("previous")}
        disabled={!track.canPrevious}
        aria-label={t.media.previous}
      >
        <Icon name="previous" size={16} />
      </button>
      <button class="main" onclick={() => send("playPause")} aria-label={t.media.playPause}>
        <Icon name={track.playing ? "pause" : "play"} size={16} />
      </button>
      <button onclick={() => send("next")} disabled={!track.canNext} aria-label={t.media.next}>
        <Icon name="next" size={16} />
      </button>
    </span>
  </div>
{/if}

<style>
  .card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-md);
  }

  /* The whole left part (art + title) opens the dance stage. */
  .stage-toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: auto;
    height: auto;
    padding: 2px;
    margin: -2px;
    text-align: left;
    border-radius: var(--radius-sm);
  }

  .stage-toggle:hover .art {
    color: #fff;
    background: var(--mod-productivity);
  }

  .staged {
    border-color: color-mix(in srgb, var(--brand) 45%, var(--island-border));
  }

  .staged .art {
    color: #fff;
    background: var(--brand);
  }

  .art {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
    color: var(--mod-productivity);
    background: color-mix(in srgb, var(--mod-productivity) 14%, transparent);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title,
  .sub {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font: var(--text-heading);
  }

  .sub {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .controls {
    display: flex;
    gap: var(--space-1);
  }

  .controls button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    color: var(--island-text);
  }

  .controls button:hover:not(:disabled) {
    background: var(--island-bg);
  }

  button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .controls .main {
    background: var(--island-text);
    color: var(--island-bg);
  }

  .controls .main:hover:not(:disabled) {
    background: var(--island-text);
    opacity: 0.9;
  }
</style>
