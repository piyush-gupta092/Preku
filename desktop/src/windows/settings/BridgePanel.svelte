<script lang="ts">
  // Settings → Modules → Phone: pair with a QR code, see the paired phone, share files, unpair.
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";
  import { formatCountdown } from "../../lib/companion";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { bridge } from "../../lib/stores/bridge.svelte";
  import type { PhoneView } from "../../lib/types/generated/PhoneView";
  import type { TransferView } from "../../lib/types/generated/TransferView";

  const TARGET_KEY = "preku.bridge.pairTarget";
  let now = $state(Date.now());
  let copied = $state(false);
  let confirming = $state<string | null>(null);
  let dragging = $state(false);
  let draft = $state("");
  let copiedText = $state<string | null>(null);
  /** Which QR to show: the Android app's, or the web companion's (iPhone and other browsers). */
  let target = $state<"app" | "web">(remembered());

  function remembered(): "app" | "web" {
    try {
      return localStorage.getItem(TARGET_KEY) === "web" ? "web" : "app";
    } catch {
      return "app";
    }
  }

  function choose(next: "app" | "web") {
    target = next;
    try {
      localStorage.setItem(TARGET_KEY, next);
    } catch {
      // Not remembered; fine.
    }
  }

  const current = $derived(bridge.state);
  const pairing = $derived(current?.pairing ?? null);
  const left = $derived(pairing ? Math.max(0, (pairing.expiresAt - now) / 1000) : 0);
  const onlinePhone = $derived(current?.phones.find((p) => p.online) ?? null);
  const transfers = $derived(current?.transfers ?? []);
  const showWeb = $derived(target === "web" && !!pairing?.webQrSvg);
  const shownLink = $derived(showWeb ? (pairing?.webLink ?? "") : (pairing?.link ?? ""));

  onMount(() => {
    let off: (() => void) | undefined;
    let offDrop: (() => void) | undefined;
    void bridge.connect().then((unlisten) => (off = unlisten));
    // Files dropped anywhere on the Settings window go to the phone.
    void (async () => {
      try {
        offDrop = await getCurrentWebview().onDragDropEvent((event) => {
          const kind = event.payload.type;
          if (kind === "enter" || kind === "over") dragging = true;
          else if (kind === "leave") dragging = false;
          else if (kind === "drop") {
            dragging = false;
            if (onlinePhone && event.payload.paths.length > 0) {
              void bridge.run(ipc.bridgeSendFiles(event.payload.paths));
            }
          }
        });
      } catch {
        // No drag and drop here (e.g. the preview harness).
      }
    })();
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => {
      clearInterval(timer);
      off?.();
      offDrop?.();
    };
  });

  async function copyLink() {
    if (!pairing) return;
    await navigator.clipboard.writeText(shownLink);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  async function sendText() {
    if (!draft.trim()) return;
    const result = await bridge.run(ipc.bridgeSendText(draft));
    if (result.ok) draft = "";
  }

  async function copyShared(id: string, text: string) {
    await navigator.clipboard.writeText(text);
    copiedText = id;
    setTimeout(() => (copiedText = null), 1500);
  }

  function ago(ms: number | null): string {
    if (!ms) return "";
    const minutes = Math.round((now - ms) / 60_000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.round(minutes / 60);
    return hours < 24 ? `${hours} h ago` : new Date(ms).toLocaleDateString();
  }

  function statusLine(phone: PhoneView): string {
    if (!phone.online) return t.bridge.lastSeen(ago(phone.lastSeen));
    const s = phone.status;
    if (!s) return t.bridge.online;
    const parts = [`🔋 ${s.battery}%${s.charging ? ` ${t.bridge.charging}` : ""}`];
    parts.push(t.bridge.networks[s.network] ?? s.network);
    return parts.join(" · ");
  }

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 ** 2) return `${Math.round(bytes / 1024)} KB`;
    if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
    return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  }

  function detail(transfer: TransferView): string {
    const peer =
      transfer.direction === "send"
        ? t.bridge.to(transfer.deviceName)
        : t.bridge.from(transfer.deviceName);
    if (transfer.status === "failed") {
      const why = transfer.error ? (t.bridge.transferErrors[transfer.error] ?? transfer.error) : "";
      return `${t.bridge.transferStatus.failed}${why ? ` (${why})` : ""} · ${peer}`;
    }
    if (transfer.status === "active")
      return `${size(transfer.bytes)} of ${size(transfer.size)} · ${peer}`;
    return `${size(transfer.size)} · ${t.bridge.transferStatus[transfer.status]} · ${peer}`;
  }
</script>

<div class="bridge">
  <p class="hint">{t.bridge.privacy}</p>

  {#each current?.phones ?? [] as phone (phone.id)}
    <div class="phone">
      <span class="dot" class:on={phone.online} aria-hidden="true"></span>
      <span class="who">
        <span class="name">📱 {phone.name}</span>
        <small
          >{phone.online ? t.bridge.online : t.bridge.offline} · {statusLine(phone)}{phone.kind ===
          "browser"
            ? ` · ${t.bridge.viaBrowser}`
            : ""}</small
        >
      </span>
      {#if confirming === phone.id}
        <span class="confirm">
          <small>{t.bridge.unpairConfirm}</small>
          <button class="danger" onclick={() => bridge.run(ipc.bridgeUnpair(phone.id))}>
            {t.bridge.unpair}
          </button>
          <button class="ghost" onclick={() => (confirming = null)}>{t.bridge.cancel}</button>
        </span>
      {:else}
        <button class="ghost" onclick={() => (confirming = phone.id)}>{t.bridge.unpair}</button>
      {/if}
    </div>
  {:else}
    {#if !pairing}<p class="muted">{t.bridge.none}</p>{/if}
  {/each}

  {#if (current?.phones.length ?? 0) > 0}
    <!-- ---- Files ---- -->
    <h3>{t.bridge.files}</h3>
    <div class="drop" class:dragging class:off={!onlinePhone}>
      {#if dragging && onlinePhone}
        <span class="drop-big">{t.bridge.dropToSend(onlinePhone.name)}</span>
      {:else if onlinePhone}
        <button class="primary" onclick={() => bridge.run(ipc.bridgePickAndSend())}>
          {t.bridge.sendFiles}
        </button>
        <span class="muted">{t.bridge.dropHere}</span>
      {:else}
        <span class="muted">{t.bridge.phoneOffline}</span>
      {/if}
    </div>
    <p class="muted">
      {onlinePhone?.kind === "browser" ? t.bridge.shareHintWeb : t.bridge.shareHint}
    </p>

    {#if transfers.length > 0}
      <ul class="transfers">
        {#each transfers as transfer (transfer.id)}
          <li class="transfer" data-status={transfer.status}>
            <span class="arrow" aria-hidden="true">{transfer.direction === "send" ? "↑" : "↓"}</span
            >
            <span class="file">
              <span class="file-name" title={transfer.name}>{transfer.name}</span>
              <small>{detail(transfer)}</small>
              {#if transfer.status === "active" || transfer.status === "waiting"}
                <span class="bar"
                  ><span
                    style:width="{transfer.size > 0 ? (transfer.bytes / transfer.size) * 100 : 0}%"
                  ></span></span
                >
              {/if}
            </span>
            {#if transfer.status === "active" || transfer.status === "waiting"}
              <button
                class="ghost"
                onclick={() => bridge.run(ipc.bridgeCancelTransfer(transfer.id))}
              >
                {t.bridge.cancelTransfer}
              </button>
            {:else if transfer.status === "done" && transfer.direction === "receive"}
              <button class="ghost" onclick={() => ipc.bridgeReveal(transfer.id)}>
                {t.bridge.show}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
    <p class="muted inbox">
      {t.bridge.received} <span class="mono">{current?.inbox}</span>
      <button class="link" onclick={() => ipc.bridgeReveal(null)}>{t.bridge.openInbox}</button>
    </p>
  {/if}

  {#if (current?.phones.length ?? 0) > 0}
    <!-- ---- Text and links ---- -->
    <h3>{t.bridge.texts}</h3>
    <form
      class="text-form"
      onsubmit={(e) => {
        e.preventDefault();
        void sendText();
      }}
    >
      <textarea
        bind:value={draft}
        rows="2"
        maxlength="20000"
        placeholder={onlinePhone ? t.bridge.textPlaceholder : t.bridge.phoneOffline}
        aria-label={t.bridge.textPlaceholder}
        disabled={!onlinePhone}
        onkeydown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void sendText();
        }}
      ></textarea>
      <button class="primary" type="submit" disabled={!onlinePhone || !draft.trim()}>
        {t.bridge.sendText}
      </button>
    </form>
    <p class="muted">{t.bridge.textHint}</p>
    {#if (current?.texts.length ?? 0) > 0}
      <ul class="transfers">
        {#each current?.texts ?? [] as shared (shared.id)}
          <li class="transfer" data-status="done">
            <span class="arrow" aria-hidden="true">{shared.direction === "send" ? "↑" : "↓"}</span>
            <span class="file">
              <span class="shared-text">{shared.text}</span>
              <small>
                {shared.direction === "send"
                  ? t.bridge.textTo(shared.deviceName)
                  : t.bridge.textFrom(shared.deviceName)} · {ago(shared.at)}
              </small>
            </span>
            {#if shared.isLink}
              <button class="ghost" onclick={() => ipc.bridgeOpenLink(shared.id)}>
                {t.bridge.open}
              </button>
            {/if}
            <button class="ghost" onclick={() => copyShared(shared.id, shared.text)}>
              {copiedText === shared.id ? t.bridge.copied : t.bridge.copy}
            </button>
          </li>
        {/each}
      </ul>
      <p class="muted inbox">
        {t.bridge.textsNote}
        <button class="link" onclick={() => bridge.run(ipc.bridgeClearTexts())}>
          {t.bridge.clearTexts}
        </button>
      </p>
    {/if}
  {/if}

  {#if pairing}
    {#if pairing.webQrSvg}
      <div class="tabs" role="tablist">
        <button
          role="tab"
          aria-selected={target === "app"}
          class:on={target === "app"}
          onclick={() => choose("app")}>{t.bridge.forApp}</button
        >
        <button
          role="tab"
          aria-selected={target === "web"}
          class:on={target === "web"}
          onclick={() => choose("web")}>{t.bridge.forWeb}</button
        >
      </div>
    {/if}
    <div class="pairing">
      <!-- The SVGs are generated by Preku itself (rects only), from the pairing links. -->
      <div class="qr" role="img" aria-label={showWeb ? t.bridge.scanWeb : t.bridge.scan}>
        {@html showWeb ? pairing.webQrSvg : pairing.qrSvg}
      </div>
      <div class="pair-text">
        <p class="say">{showWeb ? t.bridge.scanWeb : t.bridge.scan}</p>
        <p class="muted">{t.bridge.waiting}</p>
        <p class="muted mono">{showWeb ? pairing.webLink?.split("#")[0] : pairing.address}</p>
        <p class="muted">{t.bridge.expires(formatCountdown(left))}</p>
        <div class="actions">
          <button class="ghost" onclick={copyLink}>
            {copied ? t.bridge.copied : t.bridge.copyLink}
          </button>
          <button class="ghost" onclick={() => bridge.run(ipc.bridgePairCancel())}>
            {t.bridge.cancel}
          </button>
        </div>
      </div>
    </div>
    {#if showWeb}
      <p class="muted">{t.bridge.certNote} {t.bridge.homeTip}</p>
    {/if}
    <p class="muted">{t.bridge.firewall}</p>
  {:else}
    <button class="secondary" onclick={() => bridge.run(ipc.bridgePairStart())}>
      {(current?.phones.length ?? 0) > 0 ? t.bridge.pairAnother : t.bridge.pair}
    </button>
  {/if}

  {#if bridge.error || current?.error}
    <p class="error" role="alert">{bridge.error ?? current?.error}</p>
  {/if}
</div>

<style>
  .bridge {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  h3 {
    margin: var(--space-2) 0 0;
    font: 600 11px / 14px var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .hint,
  .muted {
    margin: 0;
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .phone {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
  }

  .dot.on {
    background: var(--success);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 25%, transparent);
  }

  .who,
  .file {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name,
  .file-name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font: 600 14px / 20px var(--font-ui);
  }

  small {
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  /* ---- Files ---- */
  .drop {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    min-height: 72px;
    padding: var(--space-3);
    border: 1.5px dashed color-mix(in srgb, var(--mod-bridge) 45%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--mod-bridge) 6%, transparent);
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .drop.dragging {
    border-style: solid;
    border-color: var(--mod-bridge);
    background: color-mix(in srgb, var(--mod-bridge) 16%, transparent);
  }

  .drop.off {
    border-color: var(--border);
    background: var(--surface-2);
  }

  .drop-big {
    font: 700 15px / 20px var(--font-ui);
    color: var(--mod-bridge);
  }

  .transfers {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .transfer {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }

  .arrow {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    font: 700 14px / 1 var(--font-ui);
    color: var(--mod-bridge);
    background: color-mix(in srgb, var(--mod-bridge) 14%, transparent);
    border-radius: 50%;
  }

  .transfer[data-status="failed"] .arrow {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }

  .transfer[data-status="done"] .arrow {
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, transparent);
  }

  .bar {
    height: 4px;
    overflow: hidden;
    border-radius: var(--radius-pill);
    background: var(--border);
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--mod-bridge);
    transition: width 200ms linear;
  }

  .text-form {
    display: flex;
    align-items: flex-end;
    gap: var(--space-2);
  }

  .text-form textarea {
    flex: 1;
    min-width: 0;
    padding: var(--space-2) var(--space-3);
    font: 400 14px / 20px var(--font-ui);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    resize: vertical;
  }

  .text-form textarea:focus-visible {
    outline: none;
    border-color: var(--mod-bridge);
  }

  .text-form button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .shared-text {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: 500 13px / 18px var(--font-ui);
  }

  .inbox {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
  }

  .link {
    padding: 0;
    color: var(--brand);
    font: 600 13px / 18px var(--font-ui);
  }

  .link:hover {
    text-decoration: underline;
  }

  /* ---- Pairing ---- */
  .tabs {
    display: flex;
    align-self: flex-start;
    gap: 2px;
    padding: 2px;
    background: var(--surface-2);
    border-radius: var(--radius-pill);
  }

  .tabs button {
    color: var(--text-muted);
  }

  .tabs button.on {
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }

  .pairing {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    padding: var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .qr {
    flex: none;
    width: 200px;
    height: 200px;
    padding: 6px;
    background: #ffffff;
    border-radius: var(--radius-sm);
  }

  .qr :global(svg) {
    width: 100%;
    height: 100%;
  }

  .pair-text {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .say {
    margin: 0;
    font: 600 14px / 20px var(--font-ui);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  button {
    padding: 6px var(--space-3);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .primary {
    color: #fff;
    background: var(--mod-bridge);
  }

  .secondary {
    align-self: flex-start;
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .ghost {
    flex: none;
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .danger {
    color: #fff;
    background: var(--danger);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font: 500 13px / 18px var(--font-ui);
  }
</style>
