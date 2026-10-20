<script lang="ts">
  // Pip as a wellbeing companion (replaces the module tiles while something is going on):
  // an invitation with Take a break · Later, a guided break with a countdown, or a short
  // encouragement afterwards. Pip acts it out: stretching, sipping, breathing, yawning…
  import { onMount } from "svelte";
  import {
    ACTIVITIES,
    companionMood,
    formatCountdown,
    MOODS,
    secondsLeft,
    showsUsage,
    isNudge,
    nudgeLine,
  } from "../../../lib/companion";
  import { t } from "../../../lib/i18n";
  import { companion } from "../../../lib/stores/companion.svelte";
  import { wellbeing } from "../../../lib/stores/wellbeing.svelte";
  import { inhale } from "../../../pip/expressions";
  import Pip from "../../../pip/Pip.svelte";

  let now = $state(Date.now());
  let breathing = $state(0);

  onMount(() => {
    const timer = setInterval(() => {
      now = Date.now();
      // Same clock as Pip's animation, so the words match the breath.
      breathing = inhale(performance.now());
    }, 200);
    return () => clearInterval(timer);
  });

  const session = $derived(companion.session);
  const prompt = $derived(companion.prompt);
  const kind = $derived(session?.kind ?? prompt?.kind ?? "reset");
  const phase = $derived(session?.phase ?? "invite");
  const mood = $derived(companionMood(session, prompt) ?? "happy");
  const seated = $derived(wellbeing.state?.settings.seatedMovement ?? false);

  const guide = $derived(
    kind === "stand" && seated ? t.companion.guide.standSeated : t.companion.guide[kind],
  );
  const left = $derived(session?.endsAt ? secondsLeft(session.endsAt, now) : 0);
  const total = $derived(ACTIVITIES[kind].breakS);
</script>

<section class="companion" data-phase={phase} aria-live="polite">
  <div class="pip">
    <Pip size={136} {mood} fps={45} label={t.pipLabel} />
  </div>

  <div class="body">
    {#if phase === "done"}
      <p class="say big">{t.companion.cheer[kind] ?? t.companion.cheer.default}</p>
    {:else if phase === "break"}
      <p class="say">
        {#if kind === "reset" || kind === "screenBalance"}
          <span class="breath"
            >{breathing > 0.5 ? t.companion.breatheIn : t.companion.breatheOut}</span
          >
        {/if}
        {guide}
      </p>
      <div class="timer" role="timer">
        <span class="count">{formatCountdown(left)}</span>
        <span class="bar"
          ><span style:width="{total > 0 ? (1 - left / total) * 100 : 0}%"></span></span
        >
      </div>
      <div class="actions">
        <button class="primary" onclick={() => companion.finish(kind, true)}>
          {t.companion.done}
        </button>
        <button class="ghost" onclick={() => companion.finish(kind, false)}>
          {t.companion.skip}
        </button>
      </div>
    {:else if kind === "moodCheckIn" && (phase === "ask" || prompt)}
      <p class="say">{t.companion.moodQuestion}</p>
      <div class="moods" role="group" aria-label={t.companion.moodQuestion}>
        {#each MOODS as option (option.id)}
          <button class="mood" onclick={() => companion.logMood(option.id)}>
            <span class="emoji" aria-hidden="true">{option.emoji}</span>
            {t.companion.moods[option.id]}
          </button>
        {/each}
      </div>
      <div class="actions">
        <button
          class="ghost"
          onclick={() => (prompt ? companion.later(prompt) : companion.close())}
        >
          {t.companion.later}
        </button>
      </div>
    {:else if prompt && isNudge(prompt.kind)}
      <p class="say">{nudgeLine(prompt)}</p>
      <div class="actions">
        <button class="primary" onclick={() => companion.take(prompt.kind)}>
          {prompt.kind === "water"
            ? t.wellbeing.addGlass(wellbeing.state?.settings.glassMl ?? 250)
            : t.companion.gotIt}
        </button>
        <button class="ghost" onclick={() => companion.finish(prompt.kind, false)}>
          {t.companion.notNow}
        </button>
      </div>
    {:else if prompt}
      <p class="say">{t.companion.invite[prompt.kind]}</p>
      {#if showsUsage(prompt.kind, prompt.activeMin)}
        <p class="usage">{t.companion.usage(prompt.activeMin)}</p>
      {/if}
      <div class="actions">
        <button class="primary" onclick={() => companion.take(prompt.kind)}>
          {t.companion.take[prompt.kind] ?? t.companion.takeBreak}
        </button>
        <button class="ghost" onclick={() => companion.later(prompt)}>
          {t.companion.later}
        </button>
      </div>
    {/if}
  </div>
</section>

<style>
  .companion {
    position: relative;
    display: flex;
    align-items: stretch;
    gap: var(--space-2);
    height: 156px;
    padding-right: var(--space-4);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--mod-wellbeing) 30%, var(--island-border));
    border-radius: var(--radius-md);
    background:
      radial-gradient(
        120% 120% at 0% 100%,
        color-mix(in srgb, var(--mod-wellbeing) 18%, transparent),
        transparent 60%
      ),
      var(--island-bg-raised);
  }

  .pip {
    flex: none;
    display: flex;
    align-items: flex-end;
    width: 150px;
    justify-content: center;
  }

  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: var(--space-2);
  }

  .say {
    margin: 0;
    font: 600 15px / 20px var(--font-ui);
    color: var(--island-text);
  }

  .say.big {
    font-size: 17px;
    line-height: 24px;
  }

  .breath {
    display: block;
    margin-bottom: 2px;
    font: var(--text-pill);
    color: var(--mod-wellbeing);
  }

  .usage {
    margin: 0;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
  }

  button {
    padding: 6px var(--space-3);
    font: var(--text-pill);
    border-radius: var(--radius-pill);
    transition:
      background var(--dur-fast) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }

  .primary {
    color: #10210f;
    background: var(--mod-wellbeing);
  }

  .primary:hover {
    opacity: 0.9;
  }

  .ghost {
    color: var(--island-text-muted);
    background: transparent;
    border: 1px solid var(--island-border);
  }

  .ghost:hover {
    color: var(--island-text);
    background: var(--island-bg);
  }

  .timer {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .count {
    font: 600 22px / 26px var(--font-mono);
    color: var(--island-text);
  }

  .bar {
    flex: 1;
    height: 4px;
    overflow: hidden;
    border-radius: var(--radius-pill);
    background: var(--island-bg);
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--mod-wellbeing);
    transition: width 200ms linear;
  }

  .moods {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .mood {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px var(--space-2);
    color: var(--island-text);
    background: var(--island-bg);
    border: 1px solid var(--island-border);
  }

  .mood:hover {
    border-color: var(--mod-wellbeing);
  }

  .emoji {
    font-size: 14px;
  }
</style>
