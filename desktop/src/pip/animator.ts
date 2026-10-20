import { danceAt } from "./dance";
import { expressionOf } from "./expressions";
import type { PipMood, PipPose } from "./renderer";

const BLINK_MS = 140;
const SQUISH_MS = 280;
const BREATHE_PERIOD_MS = 3200;
const DIZZY_CLICKS = 5;
const DIZZY_WINDOW_MS = 2000;
const DIZZY_MS = 1500;

/**
 * Turns time + interactions into a pose. Pure logic (no DOM), so it is unit-tested.
 * `random` is injectable to make blink timing deterministic in tests.
 */
export class PipAnimator {
  private nextBlinkAt: number;
  private blinkStartedAt = Number.NEGATIVE_INFINITY;
  private squishStartedAt = Number.NEGATIVE_INFINITY;
  private recentClicks: number[] = [];
  private dizzyUntil = Number.NEGATIVE_INFINITY;
  private lookX = 0;
  private lookY = 0;

  constructor(
    now: number,
    private readonly random: () => number = Math.random,
  ) {
    this.nextBlinkAt = now + this.blinkGap();
  }

  /** Random 3–7 s between blinks (Design.md §6). */
  private blinkGap(): number {
    return 3000 + this.random() * 4000;
  }

  poke(now: number): void {
    this.squishStartedAt = now;
    this.recentClicks = [...this.recentClicks.filter((t) => now - t < DIZZY_WINDOW_MS), now];
    if (this.recentClicks.length >= DIZZY_CLICKS) {
      this.dizzyUntil = now + DIZZY_MS;
      this.recentClicks = [];
    }
  }

  /** Point Pip's eyes; x and y are clamped to -1…1. */
  look(x: number, y: number): void {
    this.lookX = Math.max(-1, Math.min(1, x));
    this.lookY = Math.max(-1, Math.min(1, y));
  }

  isDizzy(now: number): boolean {
    return now < this.dizzyUntil;
  }

  pose(now: number, mood: PipMood, reducedMotion: boolean): PipPose {
    if (now >= this.nextBlinkAt) {
      this.blinkStartedAt = now;
      this.nextBlinkAt = now + this.blinkGap();
    }
    // No dancing with reduced motion: a happy, still Pip instead.
    const requested = mood === "dance" && reducedMotion ? "happy" : mood;
    const effectiveMood = this.isDizzy(now) ? "dizzy" : requested;
    const dancing = effectiveMood === "dance";

    return {
      mood: effectiveMood,
      breathe: reducedMotion ? 1 : breathe(now),
      squish: reducedMotion ? 1 : squish(now - this.squishStartedAt),
      hop: reducedMotion || dancing ? 0 : hop(now, effectiveMood),
      eyeOpen: blink(now - this.blinkStartedAt),
      lookX: this.lookX,
      lookY: this.lookY,
      time: now,
      still: reducedMotion,
      dance: dancing ? danceAt(now) : undefined,
    };
  }
}

export function breathe(now: number): number {
  return 1 + 0.015 * (1 - Math.cos((2 * Math.PI * now) / BREATHE_PERIOD_MS));
}

/** Eyelid closes and reopens over BLINK_MS. */
export function blink(sinceStart: number): number {
  if (sinceStart < 0 || sinceStart >= BLINK_MS) return 1;
  return 1 - Math.sin((Math.PI * sinceStart) / BLINK_MS);
}

/** 1 → 0.85 → 1.05 → 1 over SQUISH_MS (Design.md §6). */
export function squish(sinceStart: number): number {
  if (sinceStart < 0 || sinceStart >= SQUISH_MS) return 1;
  const p = sinceStart / SQUISH_MS;
  if (p < 0.35) return 1 - 0.15 * (p / 0.35);
  if (p < 0.7) return 0.85 + 0.2 * ((p - 0.35) / 0.35);
  return 1.05 - 0.05 * ((p - 0.7) / 0.3);
}

/** Hop height from the mood's expression (celebrate jumps, happy bounces gently). */
function hop(now: number, mood: PipMood): number {
  return expressionOf(mood).bob?.(now) ?? 0;
}
