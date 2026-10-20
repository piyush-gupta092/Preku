<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { productivity } from "../../../lib/stores/productivity.svelte";
  import type { ClipItem } from "../../../lib/types/generated/ClipItem";
  import { filterClips } from "./helpers";

  let query = $state("");
  let copiedId = $state<string | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  /** Image previews are fetched on demand and cached for the session. */
  let images = $state<Record<string, string>>({});

  const items = $derived(filterClips(productivity.clipboard, query));

  async function copy(item: ClipItem) {
    const result = await productivity.run(ipc.clipboardCopy(item.id));
    if (!result.ok) return;
    copiedId = item.id;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copiedId = null), 1200);
  }

  function loadImage(node: HTMLElement, id: string) {
    if (images[id]) return;
    void ipc.clipboardImage(id).then((result) => {
      if (result.ok) images[id] = result.value;
    });
    void node;
  }

  function age(createdAt: number): string {
    const minutes = Math.round((Date.now() - createdAt) / 60_000);
    const format = new Intl.RelativeTimeFormat(undefined, { numeric: "auto", style: "narrow" });
    if (minutes < 60) return format.format(-minutes, "minute");
    const hours = Math.round(minutes / 60);
    return hours < 24
      ? format.format(-hours, "hour")
      : format.format(-Math.round(hours / 24), "day");
  }
</script>

<div class="clips">
  <div class="bar">
    <label class="search">
      <Icon name="search" size={16} />
      <input bind:value={query} placeholder={t.clipboard.search} aria-label={t.clipboard.search} />
    </label>
    <button class="clear" onclick={() => productivity.run(ipc.clipboardClear())}>
      {t.clipboard.clear}
    </button>
  </div>

  <ul class="list">
    {#each items as item (item.id)}
      <li class:pinned={item.pinned}>
        <button class="content" onclick={() => copy(item)} title={t.clipboard.copy}>
          {#if item.kind === "image"}
            <span class="thumb" use:loadImage={item.id}>
              {#if images[item.id]}<img src={images[item.id]} alt="" />{/if}
            </span>
            <span class="text">{t.clipboard.image(item.width, item.height)}</span>
          {:else}
            <span class="text">{item.preview}</span>
          {/if}
          <span class="meta">{copiedId === item.id ? t.clipboard.copied : age(item.createdAt)}</span
          >
        </button>
        <span class="actions">
          <button
            class:on={item.pinned}
            onclick={() => productivity.run(ipc.clipboardSetPinned(item.id, !item.pinned))}
            aria-label={item.pinned ? t.clipboard.unpin : t.clipboard.pin}
            aria-pressed={item.pinned}
          >
            <Icon name="pin" size={14} />
          </button>
          <button
            onclick={() => productivity.run(ipc.clipboardDelete(item.id))}
            aria-label={t.clipboard.delete}
          >
            <Icon name="trash" size={14} />
          </button>
        </span>
      </li>
    {:else}
      <li class="empty">{query.trim() ? t.clipboard.noMatch : t.clipboard.empty}</li>
    {/each}
  </ul>
  <p class="privacy">{t.clipboard.privacy}</p>
</div>

<style>
  .clips {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    gap: var(--space-2);
  }

  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    color: var(--island-text-muted);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
  }

  .search:focus-within {
    border-color: var(--mod-productivity);
  }

  .search input {
    flex: 1;
    min-width: 0;
    height: 34px;
    font: var(--text-body);
    color: var(--island-text);
    background: none;
    border: none;
    outline: none;
  }

  .search input::placeholder {
    color: var(--island-text-muted);
  }

  .clear {
    padding: 0 var(--space-3);
    font: var(--text-caption);
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .clear:hover {
    color: var(--island-text);
    background: var(--island-bg-raised);
  }

  .list {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-y: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    border-radius: var(--radius-sm);
  }

  li:hover {
    background: var(--island-bg-raised);
  }

  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: var(--space-1) var(--space-2);
    text-align: left;
  }

  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .thumb {
    flex: none;
    width: 40px;
    height: 28px;
    overflow: hidden;
    border-radius: 6px;
    background: var(--island-bg);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .meta {
    flex: none;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .actions {
    display: flex;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  li:hover .actions,
  li:focus-within .actions,
  .pinned .actions {
    opacity: 1;
  }

  .actions button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .actions button:hover {
    color: var(--island-text);
    background: var(--island-bg);
  }

  .actions button.on {
    color: var(--mod-productivity);
  }

  .empty {
    justify-content: center;
    min-height: 120px;
    color: var(--island-text-muted);
    font: var(--text-caption);
  }

  .empty:hover {
    background: none;
  }

  .privacy {
    margin: 0;
    font: var(--text-caption);
    color: var(--island-text-muted);
    text-align: center;
  }
</style>
