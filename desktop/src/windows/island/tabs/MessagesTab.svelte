<script lang="ts">
  // Text and links shared with the paired phone (bridge `clipboard.text`), in the notch: send
  // one, copy one again, open a link. The same list as Settings → Phone → Text & links; it
  // lives in memory only and is gone when Preku closes.
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { bridge } from "../../../lib/stores/bridge.svelte";

  interface Props {
    /** Opens Settings → Modules → Phone (to pair, or for the full list). */
    onOpenPage: () => void;
  }
  let { onOpenPage }: Props = $props();

  let draft = $state("");
  let copiedId = $state<string | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  const current = $derived(bridge.state);
  const paired = $derived((current?.phones.length ?? 0) > 0);
  const online = $derived(current?.phones.find((phone) => phone.online) ?? null);
  const texts = $derived(current?.texts ?? []);

  async function sendText() {
    if (!draft.trim() || !online) return;
    const result = await bridge.run(ipc.bridgeSendText(draft));
    if (result.ok) draft = "";
  }

  async function copy(id: string, text: string) {
    await navigator.clipboard.writeText(text);
    copiedId = id;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copiedId = null), 1200);
  }

  function age(at: number): string {
    const minutes = Math.max(0, Math.round((Date.now() - at) / 60_000));
    const format = new Intl.RelativeTimeFormat(undefined, { numeric: "auto", style: "narrow" });
    return minutes < 60
      ? format.format(-minutes, "minute")
      : format.format(-Math.round(minutes / 60), "hour");
  }
</script>

<div class="messages">
  {#if !paired}
    <div class="empty">
      <p>{t.messages.notPaired}</p>
      <button class="open" onclick={onOpenPage}>{t.messages.pair} ›</button>
    </div>
  {:else}
    <form
      class="send"
      onsubmit={(e) => {
        e.preventDefault();
        void sendText();
      }}
    >
      <input
        bind:value={draft}
        maxlength="20000"
        placeholder={online ? t.messages.placeholder(online.name) : t.bridge.phoneOffline}
        aria-label={t.bridge.textPlaceholder}
        disabled={!online}
      />
      <button type="submit" disabled={!online || !draft.trim()}>{t.bridge.sendText}</button>
    </form>

    <ul class="list">
      {#each texts as shared (shared.id)}
        <li>
          <span class="arrow" class:out={shared.direction === "send"} aria-hidden="true">
            {shared.direction === "send" ? "↑" : "↓"}
          </span>
          <span class="body">
            <span class="text" class:link={shared.isLink} title={shared.text}>{shared.text}</span>
            <small>
              {shared.direction === "send"
                ? t.bridge.textTo(shared.deviceName)
                : t.messages.from(shared.deviceName)} · {age(shared.at)}
            </small>
          </span>
          {#if shared.isLink}
            <button class="chip" onclick={() => ipc.bridgeOpenLink(shared.id)}>
              {t.bridge.open}
            </button>
          {/if}
          <button class="chip" onclick={() => copy(shared.id, shared.text)}>
            {copiedId === shared.id ? t.bridge.copied : t.bridge.copy}
          </button>
        </li>
      {:else}
        <li class="none">{t.messages.empty}</li>
      {/each}
    </ul>
    {#if bridge.error}
      <p class="error" role="alert">{bridge.error}</p>
    {/if}
  {/if}
</div>

<style>
  .messages {
    --accent: var(--mod-bridge);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    height: 100%;
    min-height: 0;
  }

  .send {
    display: flex;
    gap: var(--space-2);
  }

  input {
    flex: 1;
    min-width: 0;
    font: var(--text-body);
    color: var(--island-text);
    background: var(--island-bg-raised);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
  }

  input::placeholder {
    color: var(--island-text-muted);
  }

  input:focus-visible {
    outline: none;
    border-color: var(--accent);
  }

  .send button {
    padding: 0 var(--space-4);
    font: var(--text-pill);
    color: var(--island-bg);
    background: var(--accent);
    border-radius: var(--radius-sm);
  }

  .send button:disabled,
  input:disabled {
    opacity: 0.45;
    cursor: default;
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
    gap: var(--space-2);
    min-height: 44px;
    padding: var(--space-1);
    border-radius: var(--radius-sm);
  }

  li:hover {
    background: var(--island-bg-raised);
  }

  .arrow {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    font: 700 12px / 1 var(--font-ui);
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, transparent);
    border-radius: 50%;
  }

  .arrow.out {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .text {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .text.link {
    color: var(--accent);
  }

  small {
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .chip {
    flex: none;
    padding: 3px 10px;
    font: var(--text-pill);
    color: var(--island-text);
    border: 1px solid var(--island-border);
    border-radius: var(--radius-pill);
  }

  .chip:hover {
    background: var(--island-bg);
  }

  .none,
  .empty {
    justify-content: center;
    color: var(--island-text-muted);
    font: var(--text-caption);
  }

  .none {
    min-height: 120px;
  }

  .none:hover {
    background: none;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    height: 100%;
    text-align: center;
  }

  .empty p {
    margin: 0;
  }

  .open {
    padding: var(--space-1) var(--space-3);
    font: var(--text-pill);
    color: var(--island-bg);
    background: var(--accent);
    border-radius: var(--radius-pill);
  }

  .error {
    margin: 0;
    font: var(--text-caption);
    color: var(--danger);
  }
</style>
