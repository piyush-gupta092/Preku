<script lang="ts">
  import Icon from "../../../lib/Icon.svelte";
  import { t } from "../../../lib/i18n";
  import { ipc } from "../../../lib/ipc";
  import { monitor } from "../../../lib/stores/monitor.svelte";
  import type { CiRun } from "../../../lib/types/generated/CiRun";
  import type { ContainerAction } from "../../../lib/types/generated/ContainerAction";
  import type { TargetKind } from "../../../lib/types/generated/TargetKind";
  import { formatClock } from "../tabs/helpers";

  let repo = $state("");
  let url = $state("");
  let token = $state("");

  const repos = $derived(monitor.targets.filter((t) => t.kind === "github"));
  const urls = $derived(monitor.targets.filter((t) => t.kind === "uptime"));

  async function add(kind: TargetKind, value: string) {
    const result = await monitor.run(ipc.targetAdd(kind, value));
    if (result.ok) {
      monitor.targets = result.value;
      if (kind === "github") repo = "";
      else url = "";
    }
  }

  async function remove(id: string) {
    const result = await monitor.run(ipc.targetDelete(id));
    if (result.ok) monitor.targets = result.value;
  }

  async function saveToken(value: string | null) {
    const result = await monitor.run(ipc.githubTokenSet(value));
    if (result.ok) {
      monitor.githubTokenSet = value !== null;
      token = "";
    }
  }

  function container(id: string, action: ContainerAction) {
    void monitor.run(ipc.dockerAction(id, action));
  }

  function ciDot(run: CiRun | null | undefined): string {
    if (!run) return "";
    if (run.status !== "completed") return "busy";
    return run.conclusion === "success" ? "ok" : run.conclusion === "failure" ? "bad" : "";
  }

  function ciLabel(run: CiRun | null | undefined): string {
    if (!run) return t.monitor.neverRun;
    if (run.status !== "completed") return t.monitor.running;
    return run.conclusion === "success" ? t.monitor.succeeded : (run.conclusion ?? "");
  }
</script>

<div class="dev pk-scroll">
  <section class="pk-section">
    <h3 class="pk-section-title"><Icon name="terminal" size={14} />{t.monitor.watch}</h3>
    <p class="pk-muted">{t.monitor.watchHint}</p>
    <p><span class="pk-code">{t.monitor.watchExample}</span></p>
    {#each monitor.watches as w (w.id)}
      <div class="pk-row">
        <span
          class="pk-dot"
          class:ok={w.state === "succeeded"}
          class:bad={w.state === "failed"}
          class:busy={w.state === "running"}
        ></span>
        <span class="pk-grow mono">{w.command}</span>
        <span class="pk-muted">
          {w.durationMs !== null ? formatClock(w.durationMs / 1000) : "⏳"}{w.state === "failed"
            ? ` · ${w.code}`
            : ""}
        </span>
      </div>
    {:else}
      <p class="pk-muted">{t.monitor.noWatches}</p>
    {/each}
  </section>

  <section class="pk-section">
    <h3 class="pk-section-title"><Icon name="box" size={14} />{t.monitor.docker}</h3>
    {#if !monitor.docker.available}
      <p class="pk-muted">{t.monitor.dockerOff}</p>
    {:else}
      {#each monitor.docker.containers as c (c.id)}
        <div class="pk-row">
          <span class="pk-dot" class:ok={c.state === "running"} class:bad={c.state === "exited"}
          ></span>
          <span class="pk-grow" title={c.image}>{c.name}</span>
          <span class="pk-muted status">{c.status}</span>
          {#if c.state === "running"}
            <button
              class="pk-icon-button"
              onclick={() => container(c.id, "restart")}
              aria-label={t.monitor.restart}
            >
              <Icon name="restart" size={14} />
            </button>
            <button
              class="pk-icon-button"
              onclick={() => container(c.id, "stop")}
              aria-label={t.monitor.stop}
            >
              <Icon name="stop" size={14} />
            </button>
          {:else}
            <button
              class="pk-icon-button"
              onclick={() => container(c.id, "start")}
              aria-label={t.monitor.start}
            >
              <Icon name="play" size={14} />
            </button>
          {/if}
        </div>
      {:else}
        <p class="pk-muted">{t.monitor.noContainers}</p>
      {/each}
    {/if}
  </section>

  <section class="pk-section">
    <h3 class="pk-section-title"><Icon name="git" size={14} />{t.monitor.github}</h3>
    {#each repos as target (target.id)}
      {@const status = monitor.ci.find((c) => c.repo === target.target)}
      <div class="pk-row">
        <span class="pk-dot {ciDot(status?.run)}"></span>
        <span class="pk-grow">{target.target}</span>
        <span class="pk-muted status" title={status?.run?.url ?? ""}>
          {status?.error ??
            `${ciLabel(status?.run)}${status?.run ? ` · ${status.run.branch}` : ""}`}
        </span>
        <button
          class="pk-icon-button"
          onclick={() => remove(target.id)}
          aria-label={t.monitor.remove}
        >
          <Icon name="trash" size={14} />
        </button>
      </div>
    {:else}
      <p class="pk-muted">{t.monitor.noRepos}</p>
    {/each}
    <form
      class="pk-add"
      onsubmit={(e) => {
        e.preventDefault();
        void add("github", repo);
      }}
    >
      <input
        class="pk-input"
        bind:value={repo}
        placeholder={t.monitor.repoPlaceholder}
        aria-label={t.monitor.repoPlaceholder}
      />
      <button class="pk-button" type="submit" disabled={!repo.trim()}>{t.monitor.add}</button>
    </form>
    {#if monitor.githubTokenSet}
      <div class="pk-row">
        <span class="pk-grow pk-muted">🔑 {t.monitor.tokenSaved}</span>
        <button class="pk-button quiet" onclick={() => saveToken(null)}
          >{t.monitor.tokenRemove}</button
        >
      </div>
    {:else}
      <form
        class="pk-add"
        onsubmit={(e) => {
          e.preventDefault();
          void saveToken(token);
        }}
      >
        <input
          class="pk-input"
          type="password"
          autocomplete="off"
          bind:value={token}
          placeholder={t.monitor.tokenPlaceholder}
          aria-label={t.monitor.tokenPlaceholder}
        />
        <button class="pk-button quiet" type="submit" disabled={!token.trim()}
          >{t.monitor.tokenSave}</button
        >
      </form>
    {/if}
  </section>

  <section class="pk-section">
    <h3 class="pk-section-title"><Icon name="globe" size={14} />{t.monitor.uptime}</h3>
    {#each urls as target (target.id)}
      {@const status = monitor.uptime.find((u) => u.url === target.target)}
      <div class="pk-row">
        <span class="pk-dot" class:ok={status?.up === true} class:bad={status?.up === false}></span>
        <span class="pk-grow">{target.target.replace(/^https?:\/\//, "")}</span>
        <span class="pk-muted status">
          {#if !status}{t.monitor.checking}{:else if status.up}{status.latencyMs} ms{:else}{status.error ??
              status.httpStatus}{/if}
        </span>
        <button
          class="pk-icon-button"
          onclick={() => remove(target.id)}
          aria-label={t.monitor.remove}
        >
          <Icon name="trash" size={14} />
        </button>
      </div>
    {:else}
      <p class="pk-muted">{t.monitor.noUrls}</p>
    {/each}
    <form
      class="pk-add"
      onsubmit={(e) => {
        e.preventDefault();
        void add("uptime", url);
      }}
    >
      <input
        class="pk-input"
        bind:value={url}
        placeholder={t.monitor.urlPlaceholder}
        aria-label={t.monitor.urlPlaceholder}
      />
      <button class="pk-button" type="submit" disabled={!url.trim()}>{t.monitor.add}</button>
    </form>
  </section>
</div>

<style>
  .dev {
    height: 100%;
  }

  p {
    margin: 0;
  }

  .mono {
    font: var(--text-mono);
  }

  .status {
    flex: none;
    max-width: 45%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
