// Pip's dance moves while music plays. Pure: (move, time) → a frame of body/arm/foot offsets,
// so every move is testable and the renderer stays simple. Tempo is a steady 120 BPM (the
// track's real tempo isn't known), and moves rotate every MOVE_MS.

export const BEAT_MS = 500;
export const MOVE_MS = 8_000;

export const DANCE_MOVES = [
  "robot",
  "gangnam",
  "bhangra",
  "floss",
  "moonwalk",
  "naatu",
  "dougie",
  "runningMan",
] as const;
export type DanceMove = (typeof DANCE_MOVES)[number];

export interface Foot {
  /** Sideways slide (box units, + = outwards). */
  dx: number;
  /** Lift off the ground (box units). */
  lift: number;
}

export interface DanceFrame {
  /** Whole-body sideways shift. */
  x: number;
  /** Bounce height. */
  y: number;
  /** Lean in radians (+ = clockwise). */
  tilt: number;
  /** Arm angles from hanging down, like `armAngle` (π = straight up). */
  armL: number;
  armR: number;
  footL: Foot;
  footR: Foot;
}

const TAU = Math.PI * 2;
/** Smooth 0…1…0 bump over one period. */
const bump = (phase: number) => (1 - Math.cos(phase * TAU)) / 2;
const frac = (x: number) => x - Math.floor(x);
const still: Foot = { dx: 0, lift: 0 };
/** 0 → 1 → -1 → 0 over one period, in straight lines (constant glide speed). */
export function triangle(p: number): number {
  const f = frac(p);
  if (f < 0.25) return f * 4;
  if (f < 0.75) return 2 - f * 4;
  return f * 4 - 4;
}

/** Which move is on at `time` (ms). */
export function currentMove(time: number): DanceMove {
  const index = Math.floor(time / MOVE_MS) % DANCE_MOVES.length;
  return DANCE_MOVES[(index + DANCE_MOVES.length) % DANCE_MOVES.length]!;
}

/** How long one move blends into the next. */
export const BLEND_MS = 350;

const lerp = (a: number, b: number, k: number) => a + (b - a) * k;
const lerpFoot = (a: Foot, b: Foot, k: number): Foot => ({
  dx: lerp(a.dx, b.dx, k),
  lift: lerp(a.lift, b.lift, k),
});

/**
 * The frame at `time`, easing from the previous move into the current one over BLEND_MS so a
 * change of move never makes Pip jump.
 */
export function danceAt(time: number): DanceFrame {
  const move = currentMove(time);
  const frame = danceFrame(move, time);
  const since = ((time % MOVE_MS) + MOVE_MS) % MOVE_MS;
  if (since >= BLEND_MS) return frame;
  const previous = danceFrame(currentMove(time - BLEND_MS), time);
  const k = (1 - Math.cos((since / BLEND_MS) * Math.PI)) / 2;
  return {
    x: lerp(previous.x, frame.x, k),
    y: lerp(previous.y, frame.y, k),
    tilt: lerp(previous.tilt, frame.tilt, k),
    armL: lerp(previous.armL, frame.armL, k),
    armR: lerp(previous.armR, frame.armR, k),
    footL: lerpFoot(previous.footL, frame.footL, k),
    footR: lerpFoot(previous.footR, frame.footR, k),
  };
}

export function danceFrame(move: DanceMove, time: number): DanceFrame {
  const beat = time / BEAT_MS;
  const phase = frac(beat);
  const half = frac(beat * 2);
  const even = Math.floor(beat) % 2 === 0;

  switch (move) {
    case "robot": {
      // Snap between poses on every half beat, no easing.
      const step = Math.floor(beat * 2) % 4;
      const poses: Array<[number, number]> = [
        [1.6, 0.3],
        [0.3, 1.6],
        [2.4, 2.4],
        [1.6, 1.6],
      ];
      const [armL, armR] = poses[step]!;
      return { x: step % 2 === 0 ? -3 : 3, y: 0, tilt: 0, armL, armR, footL: still, footR: still };
    }
    case "gangnam": {
      // Gallop: two quick hops per beat, wrists crossed low; every 4th beat the lasso arm.
      // The lasso swings up and back down within the beat, so the arm never snaps.
      const lasso = Math.floor(beat) % 4 === 3 ? Math.sin(phase * Math.PI) : 0;
      const gallop = bump(half) * 4;
      return {
        x: 0,
        y: gallop,
        tilt: 0,
        armL: 0.9,
        armR: 0.9 + lasso * (1.9 + Math.sin(time / 60) * 0.3),
        footL: { dx: 0, lift: even ? gallop : 0 },
        footR: { dx: 0, lift: even ? 0 : gallop },
      };
    }
    case "bhangra": {
      // Arms high, shoulders bouncing on the beat, hopping from foot to foot.
      const bounce = bump(phase);
      return {
        x: -2 * Math.cos(beat * Math.PI),
        y: bounce * 5,
        tilt: -0.08 * Math.cos(beat * Math.PI),
        armL: 2.7 + bounce * 0.3,
        armR: 2.7 + bounce * 0.3,
        footL: { dx: 0, lift: even ? bounce * 5 : 0 },
        footR: { dx: 0, lift: even ? 0 : bounce * 5 },
      };
    }
    case "floss": {
      // Arms swing one way while the hips go the other.
      const swing = Math.sin(beat * Math.PI);
      return {
        x: -swing * 4,
        y: 0,
        tilt: swing * 0.06,
        armL: 0.5 + swing * 0.45,
        armR: 0.5 - swing * 0.45,
        footL: still,
        footR: still,
      };
    }
    case "moonwalk": {
      // Glide one way over 2 beats, then back the other way: a smooth there-and-back
      // (centre → left → right → centre every 8 beats), never a jump back to the start.
      // One heel up while the other foot slides.
      const slide = bump(phase);
      // 0 on even beats, 1 on odd ones, easing in between.
      const heel = (1 - Math.cos(beat * Math.PI)) / 2;
      return {
        x: -8 * triangle(beat / 8),
        y: 0,
        tilt: -0.05,
        armL: 0.45,
        armR: 0.65,
        footL: { dx: even ? -slide * 3 : 0, lift: 3 * heel },
        footR: { dx: even ? 0 : slide * 3, lift: 3 * (1 - heel) },
      };
    }
    case "naatu": {
      // Very fast alternating footwork (4 steps per beat) with bouncing knees.
      const quick = frac(beat * 4);
      const left = Math.floor(beat * 4) % 2 === 0;
      const lift = bump(quick) * 4;
      return {
        x: 0,
        y: bump(half) * 2,
        tilt: 0,
        armL: 0.5,
        armR: 0.5,
        footL: { dx: left ? lift * 0.375 : 0, lift: left ? lift : 0 },
        footR: { dx: left ? 0 : lift * 0.375, lift: left ? 0 : lift },
      };
    }
    case "dougie": {
      // Sway side to side; one hand brushing over the head.
      const sway = Math.sin(beat * Math.PI);
      return {
        x: sway * 3,
        y: bump(phase) * 1.5,
        tilt: sway * 0.15,
        armL: 0.6,
        armR: 2.5 + Math.sin(beat * TAU) * 0.5,
        footL: still,
        footR: still,
      };
    }
    case "runningMan": {
      // Knee up on one side while the other foot slides back, alternating every beat.
      const lift = bump(phase) * 7;
      const slide = bump(phase) * 4;
      const pump = Math.cos(beat * Math.PI);
      return {
        x: 0,
        y: bump(phase) * 2,
        tilt: 0,
        armL: 0.75 + pump * 0.45,
        armR: 0.75 - pump * 0.45,
        footL: even ? { dx: 0, lift } : { dx: -slide, lift: 0 },
        footR: even ? { dx: slide, lift: 0 } : { dx: 0, lift },
      };
    }
  }
}
