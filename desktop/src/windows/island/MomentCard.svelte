<script lang="ts">
  // "A moment for you": once a day the notch opens on this card instead of the tiles: a quiet
  // Pip, a greeting with a word about the weather, the thought for today, Save and Dismiss.
  // Either button (or ✕) puts it away until tomorrow.
  import Pip from "../../pip/Pip.svelte";
  import Icon from "../../lib/Icon.svelte";
  import { t } from "../../lib/i18n";

  interface Props {
    /** "Good morning. It's bright and sunny out, and hot: 30° in Mumbai." */
    greeting: string;
    thought: string;
    /** "Original Preku thought", "From Kind words", "Aristotle · from Kind words". */
    source: string;
    saved: boolean;
    onsave: () => void;
    ondismiss: () => void;
  }
  let { greeting, thought, source, saved, onsave, ondismiss }: Props = $props();
</script>

<section class="moment" aria-labelledby="moment-title">
  <header>
    <span class="pip"><Pip size={52} mood="happy" fps={24} label={t.pipLabel} /></span>
    <div class="titles">
      <h2 id="moment-title">{t.moment.title}</h2>
      <span class="eyebrow">{t.moment.eyebrow}</span>
    </div>
    <button class="close" onclick={ondismiss} aria-label={t.moment.dismiss}>
      <Icon name="close" size={16} />
    </button>
  </header>

  <p class="greeting">{greeting}</p>

  <blockquote>
    <span class="mark" aria-hidden="true">“</span>
    <p>{thought}</p>
  </blockquote>
  <p class="source">{source}</p>

  <footer>
    <button class="save" class:on={saved} onclick={onsave} aria-pressed={saved}>
      <span aria-hidden="true">{saved ? "♥" : "♡"}</span>
      {saved ? t.moment.saved : t.moment.save}
    </button>
    <button class="dismiss" onclick={ondismiss}>{t.moment.dismiss}</button>
  </footer>
</section>

<style>
  .moment {
    --accent: var(--pip-body, #ffb38a);
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--space-1) var(--space-2) 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .pip {
    flex: none;
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
  }

  .titles {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  h2 {
    margin: 0;
    font: 600 18px / 24px var(--font-ui);
    color: var(--island-text);
  }

  .eyebrow {
    font: 600 10px / 14px var(--font-ui);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--island-text-muted);
  }

  .close {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .close:hover {
    color: var(--island-text);
    background: var(--island-bg-raised);
  }

  .greeting {
    margin: var(--space-2) 0 0;
    padding-left: 64px;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  blockquote {
    flex: 1;
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    margin: var(--space-3) 0 0;
    min-height: 0;
  }

  .mark {
    flex: none;
    width: 52px;
    text-align: center;
    font:
      700 52px / 44px Georgia,
      "Times New Roman",
      serif;
    color: var(--accent);
  }

  blockquote p {
    margin: 0;
    font: 500 21px / 29px var(--font-ui);
    color: var(--island-text);
    text-wrap: balance;
  }

  .source {
    margin: var(--space-1) 0 0;
    padding-left: 64px;
    font: var(--text-caption);
    color: var(--island-text-muted);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--island-border);
  }

  .save {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 34px;
    padding: 0 var(--space-4);
    font: var(--text-pill);
    color: var(--island-text);
    border: 1px solid var(--island-text-muted);
    border-radius: var(--radius-pill);
  }

  .save:hover {
    background: var(--island-bg-raised);
  }

  .save.on {
    color: var(--island-bg);
    background: var(--accent);
    border-color: var(--accent);
  }

  .dismiss {
    padding: var(--space-2) var(--space-3);
    font: var(--text-pill);
    color: var(--island-text-muted);
    border-radius: var(--radius-sm);
  }

  .dismiss:hover {
    color: var(--island-text);
  }
</style>
