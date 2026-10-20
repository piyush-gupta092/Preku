// Pip's expressions as data (Design.md §6): each mood is a recipe of eyes, brows, mouth, arms,
// hands, motion and little extras. The renderer only knows how to draw the parts, so a new
// expression is one table entry, not a new branch in every draw function.

import type { PipMood } from "./renderer";

export type EyeKind =
  | "open" // round eyes with one highlight (blinks)
  | "bright" // a bit bigger, two highlights
  | "watery" // big highlights and a wet lower edge
  | "flat" // half-lidded, flat top
  | "ring" // shocked: a ring with a tiny pupil
  | "smile" // ^ ^
  | "closed" // sleepy curves
  | "laugh" // > <
  | "wink" // left open, right ^
  | "heart"
  | "spiral";

export type MouthKind =
  | "none"
  | "small"
  | "smile"
  | "smirk"
  | "shy"
  | "grin"
  | "laugh"
  | "sing"
  | "o"
  | "bigO"
  | "frown"
  | "tremble"
  | "wavy"
  | "flat"
  | "tongue"
  | "yawn"
  | "pout"
  | "awkward";

export type BrowKind = "none" | "angry" | "worried" | "raised" | "oneUp";

/** `tears`, `sweat` sit on the face; the rest float around Pip (only drawn at ≥ 36 px). */
export type Extra =
  | "tears"
  | "sweat"
  | "hearts"
  | "confetti"
  | "question"
  | "zzz"
  | "anger"
  | "sparkle"
  | "exclaim"
  | "thought"
  | "notes";

export type Hands = "none" | "cheeks" | "chin" | "face" | "hips" | "eyes" | "cup";

export interface Expression {
  eyes: EyeKind;
  /** Eye size multiplier (1 = normal). */
  eyeScale?: number;
  /** Lid openness multiplier for round eyes (1 = fully open). */
  eyeOpen?: number;
  /** Fixed gaze (-1…1 each); otherwise the eyes follow the cursor. */
  look?: { x: number; y: number };
  mouth: MouthKind;
  brows?: BrowKind;
  /** Cheek blush strength, 0…1 (default 0.5). Above 0.75 adds blush lines. */
  blush?: number;
  /** Puffed-out cheeks (angry). */
  puff?: boolean;
  /** Leaf tips: positive droops, negative perks up. */
  leaves?: number;
  /** Leaves wiggle with excitement. */
  leafWiggle?: boolean;
  /** Arm angle from hanging down (0) to straight up (π). `side` is -1 left, 1 right. */
  arms: (time: number, side: -1 | 1) => number;
  hands?: Hands;
  /** Hop height 0…1 over time. */
  bob?: (time: number) => number;
  /** Whole-body tilt in radians (around the feet). */
  tilt?: (time: number) => number;
  /** Sideways jiggle in px (laughing, trembling, fidgeting). */
  shake?: (time: number) => number;
  /** Vertical stretch (proud stands tall, bored slouches). */
  stretch?: number;
  /** Size over time (breathing in and out, reaching up). 1 = normal. */
  breath?: (time: number) => number;
  extras?: Extra[];
}

const still = (angle: number) => () => angle;

/** One slow breath: 4 s in, 4 s out (the companion's breathing guide uses the same clock). */
export const BREATH_MS = 8000;
/** 0 (breathed out) → 1 (breathed in) → 0 over one BREATH_MS. */
export function inhale(time: number): number {
  return 0.5 - 0.5 * Math.cos((2 * Math.PI * time) / BREATH_MS);
}

/** Sipping: 0 = cup lowered, 1 = cup at the mouth; one sip every 3.2 s. */
export function sip(time: number): number {
  return 0.5 - 0.5 * Math.cos((2 * Math.PI * time) / 3200);
}
/** Arms tucked out of sight, for poses where the hands are drawn on the face instead. */
const HIDDEN = still(0.05);

export const EXPRESSIONS: Record<PipMood, Expression> = {
  // ---- Moods the backend MoodEngine picks ----
  idle: {
    eyes: "open",
    mouth: "small",
    arms: (t, side) => 0.55 + Math.sin(t / 700 + (side > 0 ? 0.6 : 0)) * 0.12,
  },
  happy: {
    eyes: "bright",
    eyeScale: 1.08,
    mouth: "smile",
    arms: (t) => 2.1 + Math.sin(t / 260) * 0.15,
    bob: (t) => 0.3 * Math.abs(Math.sin(t / 220)),
  },
  focus: { eyes: "open", eyeOpen: 0.55, mouth: "none", arms: still(0.75) },
  alert: {
    eyes: "open",
    eyeScale: 1.2,
    mouth: "o",
    leaves: -6,
    arms: still(2.4),
  },
  sleepy: {
    eyes: "closed",
    mouth: "yawn",
    leaves: 10,
    arms: still(0.2),
    extras: ["zzz"],
  },
  celebrate: {
    eyes: "smile",
    mouth: "grin",
    leaves: -6,
    leafWiggle: true,
    // Both arms up, waving out of phase.
    arms: (t, side) => 2.6 + Math.sin(t / 110 + (side > 0 ? Math.PI : 0)) * 0.4,
    bob: (t) => Math.abs(Math.sin(t / 160)),
    extras: ["confetti"],
  },
  sad: {
    eyes: "open",
    look: { x: 0, y: 1 },
    brows: "worried",
    mouth: "frown",
    leaves: 10,
    arms: still(0.1),
  },

  // ---- Frontend-only ----
  dizzy: {
    eyes: "spiral",
    mouth: "wavy",
    arms: (t, side) => 1.3 + Math.sin(t / 80 + side) * 1.0,
  },
  dance: {
    // Normally the dance frame drives the arms; a cheerful pose if drawn without one.
    eyes: "smile",
    mouth: "sing",
    leaves: -6,
    arms: still(2.2),
    extras: ["notes"],
  },

  // ---- The expression set (Settings → General → Pip's expressions) ----
  excited: {
    eyes: "bright",
    eyeScale: 1.25,
    mouth: "grin",
    leaves: -8,
    leafWiggle: true,
    arms: (t, side) => 2.7 + Math.sin(t / 90 + (side > 0 ? 1.5 : 0)) * 0.3,
    bob: (t) => Math.abs(Math.sin(t / 150)),
    extras: ["sparkle"],
  },
  laughing: {
    eyes: "laugh",
    mouth: "laugh",
    blush: 0.7,
    arms: (t) => 0.9 + Math.sin(t / 60) * 0.15,
    shake: (t) => Math.sin(t / 45) * 1.5,
  },
  crying: {
    eyes: "watery",
    look: { x: 0, y: 0.6 },
    brows: "worried",
    mouth: "tremble",
    leaves: 8,
    arms: (t) => 0.25 + Math.sin(t / 70) * 0.05,
    shake: (t) => Math.sin(t / 40) * 0.5,
    extras: ["tears"],
  },
  angry: {
    eyes: "open",
    eyeOpen: 0.5,
    brows: "angry",
    mouth: "pout",
    blush: 0.9,
    puff: true,
    leaves: -4,
    arms: still(1.0),
    shake: (t) => Math.sin(t / 35) * 0.6,
    extras: ["anger"],
  },
  confused: {
    eyes: "open",
    look: { x: 0.4, y: -0.3 },
    brows: "oneUp",
    mouth: "wavy",
    // Scratching its head with one hand.
    arms: (t, side) => (side > 0 ? 2.9 + Math.sin(t / 120) * 0.15 : 0.4),
    tilt: () => 0.15,
    extras: ["question"],
  },
  surprised: {
    eyes: "open",
    eyeScale: 1.3,
    brows: "raised",
    mouth: "o",
    leaves: -6,
    arms: still(2.3),
    extras: ["exclaim"],
  },
  shocked: {
    eyes: "ring",
    eyeScale: 1.2,
    brows: "raised",
    mouth: "bigO",
    leaves: -10,
    arms: HIDDEN,
    hands: "cheeks",
    stretch: 1.04,
    extras: ["exclaim"],
  },
  thinking: {
    eyes: "open",
    look: { x: 0.7, y: -1 },
    brows: "oneUp",
    mouth: "flat",
    arms: (_t, side) => (side > 0 ? 0.05 : 0.5),
    hands: "chin",
    extras: ["thought"],
  },
  curious: {
    eyes: "bright",
    eyeScale: 1.15,
    mouth: "small",
    leaves: -6,
    leafWiggle: true,
    arms: still(1.1),
    // Leaning in.
    tilt: (t) => 0.1 + Math.sin(t / 500) * 0.03,
  },
  shy: {
    eyes: "open",
    look: { x: -1, y: 0.5 },
    mouth: "shy",
    blush: 1,
    leaves: 4,
    arms: still(0.25),
    tilt: () => -0.06,
  },
  proud: {
    eyes: "open",
    eyeOpen: 0.7,
    look: { x: 0, y: -0.6 },
    mouth: "smirk",
    leaves: -6,
    // Hands on hips.
    arms: still(1.0),
    hands: "hips",
    stretch: 1.05,
    extras: ["sparkle"],
  },
  bored: {
    eyes: "flat",
    look: { x: 0.5, y: 0.3 },
    mouth: "flat",
    leaves: 12,
    arms: still(0.05),
    stretch: 0.93,
  },
  worried: {
    eyes: "open",
    look: { x: 0, y: 0 },
    brows: "worried",
    mouth: "wavy",
    // Fidgeting hands.
    arms: (t, side) => 0.6 + Math.sin(t / 100 + (side > 0 ? Math.PI : 0)) * 0.25,
    shake: (t) => Math.sin(t / 90) * 0.8,
    extras: ["sweat"],
  },
  playful: {
    eyes: "wink",
    mouth: "tongue",
    leafWiggle: true,
    arms: (t, side) => (side < 0 ? 2.2 + Math.sin(t / 120) * 0.3 : 0.6),
    bob: (t) => 0.2 * Math.abs(Math.sin(t / 200)),
    tilt: () => 0.08,
  },
  loving: {
    eyes: "heart",
    mouth: "smile",
    blush: 0.9,
    // Arms open for a hug.
    arms: (t) => 1.6 + Math.sin(t / 400) * 0.1,
    tilt: (t) => Math.sin(t / 600) * 0.06,
    extras: ["hearts"],
  },
  // ---- The wellbeing companion's activities ----
  restEyes: {
    // Palming: warm hands over closed eyes.
    eyes: "closed",
    mouth: "small",
    leaves: 2,
    arms: HIDDEN,
    hands: "eyes",
  },
  stretch: {
    eyes: "closed",
    mouth: "smile",
    leaves: -6,
    // A big "V" reach (big heads can't reach overhead), swaying side to side.
    arms: (t, side) => 2.3 + Math.sin(t / 1100 + (side > 0 ? Math.PI : 0)) * 0.2,
    tilt: (t) => Math.sin(t / 1100) * 0.14,
    breath: (t) => 1 + 0.035 * Math.abs(Math.sin(t / 1100)),
  },
  sip: {
    eyes: "smile",
    mouth: "none",
    blush: 0.6,
    arms: still(0.5),
    hands: "cup",
  },
  breathe: {
    eyes: "closed",
    mouth: "small",
    arms: (t) => 0.3 + 0.6 * inhale(t),
    breath: (t) => 1 + 0.05 * inhale(t),
  },
  yawn: {
    eyes: "closed",
    mouth: "yawn",
    leaves: 8,
    // One lazy arm stretched up.
    arms: (t, side) => (side > 0 ? 2.6 + Math.sin(t / 700) * 0.1 : 0.3),
    extras: ["zzz"],
  },
  embarrassed: {
    eyes: "smile",
    look: { x: -0.5, y: 0.6 },
    mouth: "awkward",
    blush: 1,
    leaves: 6,
    arms: HIDDEN,
    hands: "face",
    extras: ["sweat"],
  },
};

/** The 20 expressions shown in Settings, in the order the user listed them. */
export const GALLERY = [
  "happy",
  "excited",
  "laughing",
  "sad",
  "crying",
  "angry",
  "confused",
  "surprised",
  "shocked",
  "thinking",
  "curious",
  "shy",
  "proud",
  "sleepy",
  "bored",
  "worried",
  "playful",
  "loving",
  "embarrassed",
  "celebrate",
] as const satisfies readonly PipMood[];

export function expressionOf(mood: PipMood): Expression {
  return EXPRESSIONS[mood] ?? EXPRESSIONS.idle;
}
