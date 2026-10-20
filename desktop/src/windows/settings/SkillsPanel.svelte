<script lang="ts">
  // Settings → Skills: install skill files, switch them on and off, remove them. A skill is
  // data only (thoughts, nudges, sounds), so installing one from anywhere is safe; the
  // backend checks every file against the Preku Skill Format before keeping it.
  import { onMount } from "svelte";
  import { t } from "../../lib/i18n";
  import { ipc } from "../../lib/ipc";
  import { playNotes } from "../../lib/sound";
  import { skills } from "../../lib/stores/skills.svelte";
  import type { InstalledSkill } from "../../lib/types/generated/InstalledSkill";

  interface Props {
    /** Skill nudges may appear in the notch (the `skillNudges` activity). */
    nudgesShown: boolean;
    onnudges: (shown: boolean) => void;
  }
  let { nudgesShown, onnudges }: Props = $props();

  let confirming = $state<string | null>(null);
  let notice = $state<string | null>(null);
  /** Problems with the file just picked, one per line. */
  let problems = $state<string[]>([]);
  let installing = $state(false);

  onMount(() => {
    let off: (() => void) | undefined;
    void skills.connect().then((unlisten) => (off = unlisten));
    return () => off?.();
  });

  async function install() {
    installing = true;
    notice = null;
    problems = [];
    const result = await ipc.skillInstall();
    installing = false;
    if (result.ok) {
      skills.state = result.value.state;
      skills.error = null;
      if (result.value.installed) notice = t.skills.installed(result.value.installed);
    } else {
      problems = result.error.message.split("\n").filter(Boolean);
    }
  }

  function adds(skill: InstalledSkill): string {
    const { thoughts, nudges, sounds } = skill.manifest.contributes;
    return [
      thoughts.length && t.skills.thoughts(thoughts.length),
      nudges.length && t.skills.nudges(nudges.length),
      sounds.length && t.skills.sounds(sounds.length),
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<div class="skills">
  <p class="muted">{t.skills.intro}</p>
  <p class="safe">{t.skills.safe}</p>

  <div class="actions">
    <button class="primary" onclick={install} disabled={installing}>{t.skills.install}</button>
    {#if notice}<span class="notice" role="status">{notice}</span>{/if}
  </div>

  {#if problems.length > 0}
    <div class="problems" role="alert">
      <b>{t.skills.notInstalled}</b>
      <ul>
        {#each problems as problem, i (i)}<li>{problem}</li>{/each}
      </ul>
    </div>
  {/if}
  {#if skills.error}
    <p class="error" role="alert">{skills.error}</p>
  {/if}

  {#if skills.installed.length > 0}
    <ul class="list">
      {#each skills.installed as skill (skill.manifest.id)}
        {@const m = skill.manifest}
        <li class:off={!skill.enabled}>
          <input
            type="checkbox"
            role="switch"
            aria-label={t.skills.use(m.name)}
            checked={skill.enabled}
            onchange={(e) => skills.run(ipc.skillSetEnabled(m.id, e.currentTarget.checked))}
          />
          <div class="about">
            <span class="name">{m.name} <small>{m.version}</small></span>
            <span class="by">{t.skills.by(m.author.name)}</span>
            <p>{m.description}</p>
            <small class="adds">{adds(skill)}</small>
            {#if m.contributes.sounds.length > 0}
              <div class="sounds">
                {#each m.contributes.sounds as sound (sound.id)}
                  <button class="chip" onclick={() => playNotes(sound.notes)}>
                    ▶ {sound.name}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          <div class="end">
            {#if confirming === m.id}
              <button
                class="danger"
                onclick={() => {
                  confirming = null;
                  void skills.run(ipc.skillRemove(m.id));
                }}>{t.skills.remove}</button
              >
              <button class="ghost" onclick={() => (confirming = null)}>{t.skills.keep}</button>
            {:else}
              <button class="ghost" onclick={() => (confirming = m.id)}>{t.skills.remove}</button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">{t.skills.empty}</p>
  {/if}

  {#if skills.state.broken.length > 0}
    <h4>{t.skills.brokenTitle}</h4>
    <ul class="list">
      {#each skills.state.broken as broken (broken.file)}
        <li>
          <div class="about">
            <span class="name">{broken.file}</span>
            <small class="bad">{broken.problems[0]}</small>
          </div>
          <button class="ghost" onclick={() => skills.run(ipc.skillRemoveBroken(broken.file))}>
            {t.skills.remove}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <label class="row">
    <span class="grow">
      {t.skills.showNudges}
      <small>{t.skills.showNudgesHint}</small>
    </span>
    <input
      type="checkbox"
      role="switch"
      checked={nudgesShown}
      onchange={(e) => onnudges(e.currentTarget.checked)}
    />
  </label>

  <p class="muted">{t.skills.make}</p>
</div>

<style>
  .skills {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .muted,
  .safe,
  .empty {
    margin: 0;
    font: 400 13px / 18px var(--font-ui);
    color: var(--text-muted);
  }

  .safe {
    padding: var(--space-2) var(--space-3);
    color: var(--text);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .empty {
    padding: var(--space-4);
    text-align: center;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
  }

  h4 {
    margin: var(--space-2) 0 0;
    font: 600 11px / 14px var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  .notice {
    font: 500 13px / 18px var(--font-ui);
    color: var(--success, var(--brand));
  }

  .problems {
    padding: var(--space-3);
    font: 400 13px / 18px var(--font-ui);
    border: 1px solid var(--danger);
    border-radius: var(--radius-md);
  }

  .problems ul {
    margin: var(--space-1) 0 0;
    padding-left: var(--space-4);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .list li {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--surface-2);
    border-radius: var(--radius-md);
  }

  .list li.off .about {
    opacity: 0.55;
  }

  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 42px;
    height: 24px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--surface);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface-2);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  input[type="checkbox"][role="switch"]:checked {
    background: var(--brand);
    border-color: var(--brand);
  }

  input[type="checkbox"][role="switch"]:checked::after {
    background: #fff;
    transform: translateX(18px);
  }

  .about {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .about p {
    margin: var(--space-1) 0 0;
    font: 400 13px / 18px var(--font-ui);
  }

  .name {
    font: 600 14px / 20px var(--font-ui);
  }

  .name small,
  .by,
  .adds {
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  small.bad {
    font: 400 12px / 16px var(--font-ui);
    color: var(--danger);
  }

  .sounds {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin-top: var(--space-1);
  }

  .end {
    flex: none;
    display: flex;
    gap: var(--space-1);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font: 500 14px / 20px var(--font-ui);
  }

  .row small {
    display: block;
    font: 400 12px / 16px var(--font-ui);
    color: var(--text-muted);
  }

  .grow {
    flex: 1;
  }

  button {
    padding: 6px var(--space-3);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
  }

  .primary {
    color: #fff;
    background: var(--brand);
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ghost {
    color: var(--brand);
    border: 1px solid var(--border);
  }

  .chip {
    padding: 2px var(--space-2);
    font: 500 12px / 16px var(--font-ui);
    color: var(--text);
    border: 1px solid var(--border);
  }

  .danger {
    color: #fff;
    background: var(--danger);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font: 400 13px / 18px var(--font-ui);
  }
</style>
