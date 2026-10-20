// Short UI sounds synthesised with Web Audio: no audio files to ship or license (Design.md §8).

import type { SkillNote } from "./types/generated/SkillNote";

let context: AudioContext | null = null;

function audio(): AudioContext | null {
  try {
    context ??= new AudioContext();
    return context;
  } catch {
    return null; // Audio unavailable (e.g. no output device) — sounds are optional.
  }
}

function tone(ctx: AudioContext, frequency: number, start: number, length: number, gain: number) {
  const osc = ctx.createOscillator();
  const env = ctx.createGain();
  osc.type = "sine";
  osc.frequency.value = frequency;
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(gain, start + 0.015);
  env.gain.exponentialRampToValueAtTime(0.0001, start + length);
  osc.connect(env).connect(ctx.destination);
  osc.start(start);
  osc.stop(start + length + 0.05);
}

/** Two rising notes, ~350 ms: timer finished, task done. */
export function playComplete(): void {
  const ctx = audio();
  if (!ctx) return;
  const now = ctx.currentTime;
  tone(ctx, 784, now, 0.18, 0.12); // G5
  tone(ctx, 1046.5, now + 0.12, 0.28, 0.1); // C6
}

/** One enveloped note of another colour (the pop-up sound uses triangle waves). */
function note(
  ctx: AudioContext,
  type: OscillatorType,
  frequency: number,
  start: number,
  length: number,
  gain: number,
) {
  const osc = ctx.createOscillator();
  const env = ctx.createGain();
  osc.type = type;
  osc.frequency.value = frequency;
  env.gain.setValueAtTime(0.0001, start);
  env.gain.linearRampToValueAtTime(gain, start + 0.003);
  env.gain.exponentialRampToValueAtTime(0.0001, start + length);
  osc.connect(env).connect(ctx.destination);
  osc.start(start);
  osc.stop(start + length + 0.05);
}

/** A skill's own sound (validated by the backend: ≤12 notes, ≤2 s, gain ≤0.4). */
export function playNotes(notes: readonly SkillNote[]): void {
  const ctx = audio();
  if (!ctx) return;
  const now = ctx.currentTime + 0.02;
  for (const n of notes) note(ctx, n.wave, n.freq, now + n.at, n.length, n.gain);
}

/** The three notes of the pop-up sound: C6, E6, G6, 90 ms apart. */
export const POPUP_NOTES = [1046.5, 1318.5, 1568] as const;

/**
 * "Marimba" (the user's pick, 2026-10-11): three quick wooden notes going up, ~480 ms.
 * Played when Pip pops in with a reminder or an alert.
 */
export function playPopup(): void {
  const ctx = audio();
  if (!ctx) return;
  const now = ctx.currentTime + 0.02;
  POPUP_NOTES.forEach((frequency, i) => {
    const at = now + i * 0.09;
    note(ctx, "triangle", frequency, at, 0.26, 0.15);
    // A short high partial gives the mallet "tick".
    note(ctx, "sine", frequency * 4, at, 0.06, 0.035);
  });
}
