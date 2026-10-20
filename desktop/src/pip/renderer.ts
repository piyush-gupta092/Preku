// Draws Pip (Design.md §6) with plain Canvas 2D shapes — no sprites, no animation libraries.
// Three bodies (`bodies.ts`) share one face, the expression recipes (`expressions.ts`) and the
// dances (`dance.ts`). Arms and legs are two-segment limbs: forward kinematics from a "raise"
// angle, or inverse kinematics when a hand or foot has to reach a point (cheeks, chin, steps).

import type { MascotStyle } from "../lib/types/generated/MascotStyle";
import type { PipMood as BackendMood } from "../lib/types/generated/PipMood";
import { armFromAngle, bodyFor, OUTLINE, solveLimb, type Body, type Point } from "./bodies";
import type { DanceFrame } from "./dance";
import { expressionOf, sip, type Expression, type EyeKind } from "./expressions";
import type { PipPalette } from "./palette";

export type { PipPalette } from "./palette";

/**
 * Moods chosen by the backend MoodEngine, plus frontend-only ones: `dizzy` (poked too often),
 * `dance` (music is playing) and the expression set shown in Settings.
 */
export type PipMood =
  | BackendMood
  | "dizzy"
  | "dance"
  | "excited"
  | "laughing"
  | "crying"
  | "angry"
  | "confused"
  | "surprised"
  | "shocked"
  | "thinking"
  | "curious"
  | "shy"
  | "proud"
  | "bored"
  | "worried"
  | "playful"
  | "loving"
  | "embarrassed"
  // Wellbeing companion activities.
  | "restEyes"
  | "stretch"
  | "sip"
  | "breathe"
  | "yawn";

export interface PipPose {
  mood: PipMood;
  /** Uniform breathing scale, ~1.0–1.03. */
  breathe: number;
  /** Vertical squash (1 = none, <1 squashed, >1 stretched). */
  squish: number;
  /** 0 = on the ground, 1 = top of a hop. */
  hop: number;
  /** Eyelid openness, 0 = closed, 1 = open. */
  eyeOpen: number;
  /** Where Pip looks, each in -1…1. */
  lookX: number;
  lookY: number;
  /** Milliseconds, for moods with their own motion (dizzy spirals, tears, z…). */
  time: number;
  /** Reduced motion: no shaking, tilting or drifting extras. */
  still?: boolean;
  /** Set while dancing: body/arm/foot offsets for this frame. */
  dance?: DanceFrame;
}

/** Full figures are laid out in a box this many units tall (ground near the bottom). */
const BOX = 270;
const GROUND = 0.97;
/** Below this size only the head is drawn (the 24 px pill). */
export const BUST_BELOW = 36;
/** Dances and shakes were tuned for the old 100-unit Pip; this scales them to the new bodies. */
const MOTION = 2.5;
const HOP_HEIGHT = 28;

// The shared face is drawn in the original face coordinates (eyes at y = -44), then placed on
// each body's head with `body.face`.
const EYE_DX = 13;
const EYE_Y = -44;
const FACE_Y = -30; // cheeks and mouth

// Effect colors that read on both the dark island and light Settings.
const TEAR = "#4fb0ff";
const HEART = "#ff4f6d";
const TONGUE = "#ff7a8a";
const ACCENT = "#8b7bff";
const ANGER = "#ff453a";
const SPARK = "#ffc93c";
const CONFETTI = ["#ffb020", "#3da5ff", "#22c7b8", "#5bd16e", "#ff7a7a", "#a78bfa"];

/**
 * Arm "raise" angle in radians, from hanging straight down (0) to straight up (π). `side` is
 * -1 (left) or 1 (right). Pure, so moods are easy to tune and test.
 */
export function armAngle(mood: PipMood, time: number, side: -1 | 1): number {
  return expressionOf(mood).arms(time, side);
}

/** Face coordinates (eyes at y = -44) → body coordinates. */
function facePoint(body: Body, x: number, y: number): Point {
  return { x: body.face.x + x * body.face.scale, y: body.face.y + (y - EYE_Y) * body.face.scale };
}

/** Where a hand should go for expressions that put hands on the face or hips. */
function handTarget(body: Body, spec: Expression, side: -1 | 1, t: number): Point | null {
  switch (spec.hands ?? "none") {
    case "eyes":
      return facePoint(body, side * EYE_DX, EYE_Y + 2);
    case "cup": {
      // The right hand lifts a cup between the lap and the mouth.
      if (side < 0) return null;
      const k = sip(t);
      const low = facePoint(body, 18, -4);
      const mouth = facePoint(body, 6, -26);
      return { x: low.x + (mouth.x - low.x) * k, y: low.y + (mouth.y - low.y) * k };
    }
    case "cheeks":
      return facePoint(body, side * 22, -27);
    case "face":
      return facePoint(body, side * 17, -30);
    case "chin":
      return side > 0 ? facePoint(body, 7, -20) : null;
    case "hips":
      return { x: side * (body.shoulder.x + 4), y: body.shoulder.y + 26 };
    default:
      return null;
  }
}

export function drawPip(
  ctx: CanvasRenderingContext2D,
  size: number,
  pose: PipPose,
  basePalette: PipPalette,
  style?: MascotStyle,
): void {
  const body = bodyFor(style);
  // Classic Pip has soft edges: a faint shade instead of a dark outline (hands on its face
  // still show).
  const palette =
    body.outline === false ? { ...basePalette, outline: basePalette.shade } : basePalette;
  const spec = expressionOf(pose.mood);
  // Reduced motion freezes the extra motion at its resting value.
  const t = pose.still ? 0 : pose.time;
  const bust = size < BUST_BELOW;

  ctx.clearRect(0, 0, size, size);
  ctx.save();
  if (bust) {
    // Head only (or the whole of a small figure), filling the canvas.
    const k = size / body.bust.h;
    ctx.translate(size / 2, size / 2);
    ctx.scale(k, k);
    ctx.translate(0, -body.bust.cy);
  } else {
    const k = size / BOX;
    ctx.translate(size / 2, size * GROUND);
    ctx.scale(k, k);
  }
  if (pose.dance) {
    ctx.translate(pose.dance.x * MOTION, -pose.dance.y * MOTION);
    ctx.rotate(pose.dance.tilt);
  } else {
    ctx.translate((spec.shake?.(t) ?? 0) * MOTION, bust ? 0 : -pose.hop * HOP_HEIGHT);
    ctx.rotate(spec.tilt?.(t) ?? 0);
  }
  // Squash keeps volume: shorter means wider.
  const stretch = spec.stretch ?? 1;
  ctx.scale(pose.breathe * (2 - pose.squish), pose.breathe * pose.squish * stretch);
  const breath = spec.breath?.(t) ?? 1;
  ctx.scale(1 + (breath - 1) * 0.5, breath);

  if (!bust || body.wholeInBust) {
    const arms = ([-1, 1] as const).map((side) => armFor(body, spec, pose, side));
    drawLegs(ctx, body, pose, palette);
    for (const arm of arms) if (!arm.front) drawArm(ctx, body, arm, palette);
    body.drawTorso(ctx, palette);
    drawLeaves(ctx, body, spec, t, palette);
    body.drawHead(ctx, palette);
    drawFace(ctx, body, pose, spec, t, palette);
    for (const arm of arms) {
      if (!arm.front) continue;
      if (spec.hands === "cup" && arm.side > 0) {
        // Arm, then the cup, then the hand wrapped around it.
        noodle(ctx, arm.start, arm.mid, arm.end, body.armWidth, palette);
        drawCup(ctx, body, arm, palette, sip(t));
        drawHand(ctx, body, arm, palette);
      } else {
        drawArm(ctx, body, arm, palette);
      }
    }
    // Floating extras only where there's room to see them (not in the 24 px pill).
    if (!bust) drawFloating(ctx, body, spec, t, palette);
  } else {
    drawLeaves(ctx, body, spec, t, palette);
    body.drawHead(ctx, palette);
    drawFace(ctx, body, pose, spec, t, palette);
  }
  ctx.restore();
}

function drawFace(
  ctx: CanvasRenderingContext2D,
  body: Body,
  pose: PipPose,
  spec: Expression,
  t: number,
  palette: PipPalette,
) {
  ctx.save();
  ctx.translate(body.face.x, body.face.y);
  ctx.scale(body.face.scale, body.face.scale);
  ctx.translate(0, -EYE_Y);
  drawCheeks(ctx, spec, palette);
  drawEyes(ctx, pose, spec, palette);
  drawBrows(ctx, spec, palette);
  drawMouth(ctx, spec, t, palette);
  if (spec.extras?.includes("tears")) drawTears(ctx, t);
  if (spec.extras?.includes("sweat")) drawSweat(ctx, t);
  ctx.restore();
}

interface ArmPose {
  side: -1 | 1;
  start: Point;
  mid: Point;
  end: Point;
  /** Raised arms and hands on the face/hips are drawn in front of the body. */
  front: boolean;
}

function armFor(body: Body, spec: Expression, pose: PipPose, side: -1 | 1): ArmPose {
  const start = { x: side * body.shoulder.x, y: body.shoulder.y };
  const target = pose.dance ? null : handTarget(body, spec, side, pose.still ? 0 : pose.time);
  if (target) {
    return { side, start, ...solveLimb(start, target, body.arm, side), front: true };
  }
  const angle = pose.dance
    ? side < 0
      ? pose.dance.armL
      : pose.dance.armR
    : spec.arms(pose.still ? 0 : pose.time, side);
  const arm = armFromAngle(start, angle, body.arm, side);
  return { side, start, mid: arm.mid, end: arm.end, front: arm.upper > 1.1 };
}

/** A soft "noodle" limb through the elbow/knee: an outline pass, then the body color. */
function noodle(
  ctx: CanvasRenderingContext2D,
  start: Point,
  mid: Point,
  end: Point,
  width: number,
  palette: PipPalette,
) {
  // The curve's control point is pushed past the joint so the limb really bends through it.
  const cx = 2 * mid.x - (start.x + end.x) / 2;
  const cy = 2 * mid.y - (start.y + end.y) / 2;
  ctx.lineCap = "round";
  for (const [color, w] of [
    [palette.outline, width + OUTLINE * 1.6],
    [palette.body, width],
  ] as const) {
    ctx.strokeStyle = color;
    ctx.lineWidth = w;
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.quadraticCurveTo(cx, cy, end.x, end.y);
    ctx.stroke();
  }
}

function drawArm(ctx: CanvasRenderingContext2D, body: Body, arm: ArmPose, palette: PipPalette) {
  noodle(ctx, arm.start, arm.mid, arm.end, body.armWidth, palette);
  drawHand(ctx, body, arm, palette);
}

/** A little cup of water held in the hand, tipping as it reaches the mouth. */
function drawCup(
  ctx: CanvasRenderingContext2D,
  body: Body,
  arm: ArmPose,
  palette: PipPalette,
  lifted: number,
) {
  const r = body.handRadius;
  const w = r * 2.8;
  const h = r * 3.2;
  ctx.save();
  // Held in front of the hand, which then wraps around its side.
  ctx.translate(arm.end.x - r * 1.2, arm.end.y - r * 0.4);
  ctx.rotate(-lifted * 0.6);
  ctx.lineWidth = OUTLINE * 0.75;
  ctx.strokeStyle = palette.outline;
  ctx.lineJoin = "round";
  // Handle on the outside.
  ctx.beginPath();
  ctx.arc(w * 0.55, 0, h * 0.22, -Math.PI / 2, Math.PI / 2);
  ctx.stroke();
  // Cup body (slightly narrower at the bottom).
  ctx.beginPath();
  ctx.moveTo(-w / 2, -h / 2);
  ctx.lineTo(w / 2, -h / 2);
  ctx.lineTo(w * 0.4, h / 2);
  ctx.lineTo(-w * 0.4, h / 2);
  ctx.closePath();
  ctx.fillStyle = "#f7f5ff";
  ctx.fill();
  ctx.stroke();
  // Water.
  ctx.fillStyle = "#7cc8ff";
  ctx.beginPath();
  ctx.ellipse(0, -h / 2 + 3, w * 0.42, 3, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function drawHand(ctx: CanvasRenderingContext2D, body: Body, arm: ArmPose, palette: PipPalette) {
  // Mitten hand with a little thumb pointing back along the forearm's inside.
  const r = body.handRadius;
  const dir = Math.atan2(arm.end.y - arm.mid.y, arm.end.x - arm.mid.x);
  const thumb = dir - arm.side * 1.2;
  ctx.lineWidth = OUTLINE * 0.85;
  ctx.strokeStyle = palette.outline;
  ctx.fillStyle = palette.body;
  ctx.beginPath();
  ctx.arc(
    arm.end.x + Math.cos(thumb) * r * 0.85,
    arm.end.y + Math.sin(thumb) * r * 0.85,
    r * 0.45,
    0,
    Math.PI * 2,
  );
  ctx.fill();
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(arm.end.x, arm.end.y, r, 0, Math.PI * 2);
  ctx.fill();
  ctx.stroke();
}

/** Legs reach for their feet: resting on the ground, tucked while hopping, stepping in dances. */
function drawLegs(ctx: CanvasRenderingContext2D, body: Body, pose: PipPose, palette: PipPalette) {
  const [rx, ry] = body.foot;
  for (const side of [-1, 1] as const) {
    const step = pose.dance ? (side < 0 ? pose.dance.footL : pose.dance.footR) : null;
    const hip = { x: side * body.hip.x, y: body.hip.y };
    const target = {
      x: side * (body.hip.x + 3 + (step?.dx ?? 0) * MOTION),
      y: -ry * 1.4 - (step?.lift ?? 0) * MOTION - pose.hop * 10,
    };
    const leg = solveLimb(hip, target, body.leg, side);
    noodle(ctx, hip, leg.mid, leg.end, body.legWidth, palette);
    // Shoe, toes pointing slightly outwards.
    const x = leg.end.x + side * rx * 0.3;
    const y = leg.end.y + ry * 0.4;
    ctx.beginPath();
    ctx.ellipse(x, y, rx, ry, 0, 0, Math.PI * 2);
    ctx.fillStyle = palette.body;
    ctx.fill();
    ctx.save();
    ctx.clip();
    ctx.fillStyle = palette.shade;
    ctx.beginPath();
    ctx.ellipse(x, y + ry * 0.7, rx, ry * 0.5, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
    ctx.beginPath();
    ctx.ellipse(x, y, rx, ry, 0, 0, Math.PI * 2);
    ctx.lineWidth = OUTLINE * 0.85;
    ctx.strokeStyle = palette.outline;
    ctx.stroke();
  }
}

/** Two outlined leaves; they droop or perk with the mood and wiggle when excited. */
function drawLeaves(
  ctx: CanvasRenderingContext2D,
  body: Body,
  spec: Expression,
  t: number,
  palette: PipPalette,
) {
  const s = body.leaves.scale;
  for (const side of [-1, 1] as const) {
    const wiggle = spec.leafWiggle ? Math.sin(t / 90 + side) * 0.12 : 0;
    ctx.save();
    ctx.translate(body.leaves.x, body.leaves.y);
    ctx.rotate(side * (spec.leaves ?? 0) * 0.03 + wiggle);
    ctx.scale(s, s);
    ctx.beginPath();
    ctx.moveTo(0, 0);
    ctx.bezierCurveTo(side * 6, -30, side * 34, -34, side * 40, -22);
    ctx.bezierCurveTo(side * 34, -6, side * 14, 2, 0, 0);
    ctx.closePath();
    ctx.fillStyle = palette.leaf;
    ctx.fill();
    ctx.lineWidth = OUTLINE * 0.85;
    ctx.lineJoin = "round";
    ctx.strokeStyle = palette.outline;
    ctx.stroke();
    ctx.strokeStyle = palette.leafVein;
    ctx.lineWidth = 2.5;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(side * 4, -4);
    ctx.quadraticCurveTo(side * 20, -18, side * 34, -21);
    ctx.stroke();
    ctx.restore();
  }
}

function drawCheeks(ctx: CanvasRenderingContext2D, spec: Expression, palette: PipPalette) {
  const blush = spec.blush ?? 0.5;
  const r = spec.puff ? 7.5 : blush > 0.75 ? 6.5 : 5.5;
  ctx.save();
  for (const side of [-1, 1]) {
    ctx.globalAlpha = blush;
    ctx.fillStyle = palette.cheek;
    ctx.beginPath();
    ctx.arc(side * 24, FACE_Y, r, 0, Math.PI * 2);
    ctx.fill();
    if (blush > 0.75 && !spec.puff) {
      // Little blush strokes.
      ctx.globalAlpha = 0.45;
      ctx.strokeStyle = palette.eye;
      ctx.lineWidth = 1.2;
      for (const dx of [-3, 0, 3]) {
        ctx.beginPath();
        ctx.moveTo(side * 24 + dx - 1, FACE_Y + 2);
        ctx.lineTo(side * 24 + dx + 1, FACE_Y - 2);
        ctx.stroke();
      }
    }
  }
  ctx.restore();
}

function drawEyes(
  ctx: CanvasRenderingContext2D,
  pose: PipPose,
  spec: Expression,
  palette: PipPalette,
) {
  const look = spec.look ?? { x: pose.lookX, y: pose.lookY };
  const lookX = look.x * 3;
  const lookY = look.y * 2.5;
  ctx.fillStyle = palette.eye;
  ctx.strokeStyle = palette.eye;
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  for (const side of [-1, 1] as const) {
    const x = side * EYE_DX + lookX;
    const y = EYE_Y + lookY;
    let kind: EyeKind = spec.eyes;
    if (kind === "wink") kind = side > 0 ? "smile" : "bright";

    switch (kind) {
      case "smile":
        ctx.beginPath();
        ctx.arc(x, y + 3, 5, Math.PI * 1.15, Math.PI * 1.85);
        ctx.stroke();
        continue;
      case "closed":
        ctx.beginPath();
        ctx.moveTo(x - 5, y + 1);
        ctx.quadraticCurveTo(x, y + 4, x + 5, y + 1);
        ctx.stroke();
        continue;
      case "laugh":
        // > <
        ctx.beginPath();
        ctx.moveTo(x - side * 4, y - 4);
        ctx.lineTo(x + side * 3, y);
        ctx.lineTo(x - side * 4, y + 4);
        ctx.stroke();
        continue;
      case "spiral":
        drawSpiral(ctx, x, y, pose.time * side);
        continue;
      case "heart":
        ctx.fillStyle = HEART;
        drawHeart(ctx, x, y + 1, 12);
        ctx.fillStyle = palette.eye;
        continue;
      case "ring": {
        const r = 6 * (spec.eyeScale ?? 1);
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(x, y, 2, 0, Math.PI * 2);
        ctx.fill();
        ctx.lineWidth = 3;
        continue;
      }
    }

    // Round eyes: open, bright, watery, flat.
    const big = spec.eyeScale ?? 1;
    const lid = kind === "flat" ? 0.45 : (spec.eyeOpen ?? 1);
    const open = pose.eyeOpen * lid;
    const rx = 5.5 * big;
    const ry = 8 * big * Math.max(open, 0.08);
    ctx.beginPath();
    ctx.ellipse(x, y, rx, ry, 0, 0, Math.PI * 2);
    ctx.fill();

    if (kind === "flat") {
      // A heavy, straight eyelid.
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.moveTo(x - rx - 1.5, y - ry);
      ctx.lineTo(x + rx + 1.5, y - ry);
      ctx.stroke();
      ctx.lineWidth = 3;
    }
    if (open > 0.5) {
      const shine = kind === "watery" ? 2.6 : kind === "bright" ? 2.2 : 1.7;
      ctx.fillStyle = palette.highlight;
      ctx.beginPath();
      ctx.arc(x + 1.8, y - ry * 0.4, shine, 0, Math.PI * 2);
      ctx.fill();
      if (kind === "bright" || kind === "watery") {
        ctx.beginPath();
        ctx.arc(x - 1.6, y + ry * 0.35, shine * 0.5, 0, Math.PI * 2);
        ctx.fill();
      }
      if (kind === "watery") {
        ctx.fillStyle = "rgba(120, 190, 255, 0.75)";
        ctx.beginPath();
        ctx.ellipse(x, y + ry * 0.6, rx * 0.85, ry * 0.32, 0, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.fillStyle = palette.eye;
    }
  }
}

function drawBrows(ctx: CanvasRenderingContext2D, spec: Expression, palette: PipPalette) {
  const brows = spec.brows ?? "none";
  if (brows === "none") return;
  const top = EYE_Y - 8 * (spec.eyeScale ?? 1); // top of the eye
  ctx.strokeStyle = palette.eye;
  ctx.lineWidth = 2.6;
  ctx.lineCap = "round";
  for (const side of [-1, 1] as const) {
    const inner = side * 6;
    const outer = side * 20;
    ctx.beginPath();
    switch (brows) {
      case "angry":
        ctx.moveTo(inner, top - 2);
        ctx.lineTo(outer, top - 8);
        break;
      case "worried":
        ctx.moveTo(inner, top - 9);
        ctx.lineTo(outer, top - 4);
        break;
      case "raised":
        ctx.arc(side * EYE_DX, top - 2, 6, Math.PI * 1.2, Math.PI * 1.8);
        break;
      case "oneUp":
        if (side > 0) ctx.arc(side * EYE_DX, top - 3, 6, Math.PI * 1.2, Math.PI * 1.8);
        else {
          ctx.moveTo(inner, top - 4);
          ctx.lineTo(outer, top - 4);
        }
        break;
    }
    ctx.stroke();
  }
}

function drawSpiral(ctx: CanvasRenderingContext2D, x: number, y: number, time: number) {
  const turn = time / 120;
  ctx.beginPath();
  for (let i = 0; i <= 24; i++) {
    const a = turn + (i / 24) * Math.PI * 3;
    const r = (i / 24) * 6;
    const px = x + Math.cos(a) * r;
    const py = y + Math.sin(a) * r;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.stroke();
}

/** A heart centred on (x, y), `size` wide. */
function drawHeart(ctx: CanvasRenderingContext2D, x: number, y: number, size: number) {
  const s = size / 2;
  ctx.beginPath();
  ctx.moveTo(x, y + s * 0.9);
  ctx.bezierCurveTo(x - s * 1.6, y - s * 0.1, x - s * 0.7, y - s * 1.3, x, y - s * 0.45);
  ctx.bezierCurveTo(x + s * 0.7, y - s * 1.3, x + s * 1.6, y - s * 0.1, x, y + s * 0.9);
  ctx.fill();
}

function drawMouth(
  ctx: CanvasRenderingContext2D,
  spec: Expression,
  t: number,
  palette: PipPalette,
) {
  ctx.strokeStyle = palette.eye;
  ctx.fillStyle = palette.eye;
  ctx.lineWidth = 2.5;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  const y = FACE_Y;

  ctx.beginPath();
  switch (spec.mouth) {
    case "none":
      return;
    case "small":
      ctx.arc(0, y - 2, 3, Math.PI * 0.15, Math.PI * 0.85);
      ctx.stroke();
      return;
    case "smile":
      ctx.arc(0, y - 3, 6, Math.PI * 0.15, Math.PI * 0.85);
      ctx.stroke();
      return;
    case "smirk":
      ctx.moveTo(-4, y);
      ctx.quadraticCurveTo(1, y + 3, 6, y - 2);
      ctx.stroke();
      return;
    case "shy":
      ctx.arc(-3, y - 1, 2.5, Math.PI * 0.15, Math.PI * 0.85);
      ctx.stroke();
      return;
    case "grin":
    case "laugh": {
      // Open "D" smile with a tongue.
      const r = spec.mouth === "laugh" ? 6 + Math.abs(Math.sin(t / 90)) * 1.2 : 6;
      ctx.arc(0, y - 2, r, 0, Math.PI);
      ctx.closePath();
      ctx.fill();
      ctx.save();
      ctx.clip();
      ctx.fillStyle = TONGUE;
      ctx.beginPath();
      ctx.ellipse(0, y - 2 + r, r * 0.6, r * 0.45, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
      return;
    }
    case "sing":
      ctx.ellipse(0, y - 1, 3.5, 3 + Math.abs(Math.sin(t / 250)) * 1.5, 0, 0, Math.PI * 2);
      ctx.fill();
      return;
    case "o":
      ctx.ellipse(0, y, 2.8, 3.4, 0, 0, Math.PI * 2);
      ctx.fill();
      return;
    case "bigO":
      ctx.ellipse(0, y + 1, 3.8, 5.5, 0, 0, Math.PI * 2);
      ctx.fill();
      return;
    case "frown":
      ctx.arc(0, y + 3, 4, Math.PI * 1.2, Math.PI * 1.8);
      ctx.stroke();
      return;
    case "pout":
      ctx.arc(0, y + 2, 3, Math.PI * 1.2, Math.PI * 1.8);
      ctx.stroke();
      return;
    case "tremble":
      // A wobbly frown.
      for (let i = 0; i <= 10; i++) {
        const px = -5 + i;
        const py = y + 2 - (1 - (px / 5) ** 2) * 2.5 + Math.sin(px * 2 + t / 40) * 0.6;
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.stroke();
      return;
    case "wavy":
      ctx.moveTo(-4, y);
      ctx.quadraticCurveTo(-2, y - 2, 0, y);
      ctx.quadraticCurveTo(2, y + 2, 4, y);
      ctx.stroke();
      return;
    case "flat":
      ctx.moveTo(-4, y);
      ctx.lineTo(4, y);
      ctx.stroke();
      return;
    case "awkward":
      ctx.moveTo(-5, y - 1);
      ctx.lineTo(-2.5, y + 1);
      ctx.lineTo(0, y - 1);
      ctx.lineTo(2.5, y + 1);
      ctx.lineTo(5, y - 1);
      ctx.stroke();
      return;
    case "tongue":
      // A cheeky grin with the tongue poking out under it.
      ctx.fillStyle = TONGUE;
      ctx.ellipse(2.5, y + 4, 3.4, 4.2, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.lineWidth = 1.4;
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(2.5, y + 2);
      ctx.lineTo(2.5, y + 5);
      ctx.stroke();
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.arc(0, y - 3, 6, Math.PI * 0.12, Math.PI * 0.88);
      ctx.stroke();
      return;
    case "yawn": {
      // A slow yawn every 4 s; otherwise a sleepy closed mouth.
      const phase = (t % 4000) / 4000;
      const open = phase < 0.4 ? Math.sin((phase / 0.4) * Math.PI) : 0;
      if (open > 0.05) {
        ctx.ellipse(0, y + 1, 2.5 + open * 1.5, 1 + open * 4.5, 0, 0, Math.PI * 2);
        ctx.fill();
      }
      return;
    }
  }
}

/** Tears rolling down from both eyes. */
function drawTears(ctx: CanvasRenderingContext2D, t: number) {
  ctx.save();
  ctx.fillStyle = TEAR;
  for (const side of [-1, 1]) {
    const p = (((t / 900 + (side > 0 ? 0.5 : 0)) % 1) + 1) % 1;
    const x = side * (EYE_DX + 4);
    const y = EYE_Y + 8 + p * 16;
    ctx.globalAlpha = 1 - p * 0.7;
    drawDrop(ctx, x, y, 2.4);
  }
  ctx.restore();
}

/** A nervous sweat drop beside the head. */
function drawSweat(ctx: CanvasRenderingContext2D, t: number) {
  ctx.save();
  ctx.fillStyle = TEAR;
  ctx.globalAlpha = 0.9;
  drawDrop(ctx, 30, -62 + ((t / 60) % 5), 3.4);
  ctx.fillStyle = "#ffffff";
  ctx.beginPath();
  ctx.arc(29, -61 + ((t / 60) % 5), 0.9, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function drawDrop(ctx: CanvasRenderingContext2D, x: number, y: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x, y - r * 2.2);
  ctx.quadraticCurveTo(x + r * 1.1, y - r * 0.4, x + r, y);
  ctx.arc(x, y, r, 0, Math.PI);
  ctx.quadraticCurveTo(x - r * 1.1, y - r * 0.4, x, y - r * 2.2);
  ctx.fill();
}

/**
 * Things floating around Pip's head. Placed in the original 100-unit layout (head centre at
 * 50, 62.6, radius 31) and mapped onto this body's head.
 */
function drawFloating(
  ctx: CanvasRenderingContext2D,
  body: Body,
  spec: Expression,
  t: number,
  palette: PipPalette,
) {
  const extras = spec.extras ?? [];
  if (extras.length === 0) return;
  ctx.save();
  const k = body.head.r / 31;
  ctx.translate(body.head.x, body.head.y);
  ctx.scale(k, k);
  ctx.translate(-50, -62.6);
  ctx.textAlign = "center";
  ctx.lineCap = "round";

  for (const extra of extras) {
    ctx.save();
    switch (extra) {
      case "notes":
        // Two ♪ notes drifting up beside the head while dancing.
        ctx.fillStyle = palette.leaf;
        ctx.font = "bold 16px sans-serif";
        for (const [offset, x] of [
          [0, 84],
          [0.5, 14],
        ] as const) {
          const p = (t / 1600 + offset) % 1;
          ctx.globalAlpha = Math.sin(p * Math.PI) * 0.9;
          ctx.fillText(offset === 0 ? "♪" : "♫", x + Math.sin(p * 6) * 3, 44 - p * 34);
        }
        break;
      case "hearts":
        ctx.fillStyle = HEART;
        for (const [offset, x] of [
          [0, 84],
          [0.5, 16],
        ] as const) {
          const p = (t / 1800 + offset) % 1;
          ctx.globalAlpha = Math.sin(p * Math.PI);
          drawHeart(ctx, x + Math.sin(p * 6) * 3, 40 - p * 32, 8 + p * 3);
        }
        break;
      case "zzz":
        ctx.fillStyle = ACCENT;
        for (let i = 0; i < 3; i++) {
          const p = (t / 2400 + i / 3) % 1;
          ctx.globalAlpha = Math.sin(p * Math.PI);
          ctx.font = `bold ${8 + p * 8}px sans-serif`;
          ctx.fillText("z", 74 + p * 16, 34 - p * 26);
        }
        break;
      case "question": {
        ctx.fillStyle = ACCENT;
        ctx.font = "bold 20px sans-serif";
        ctx.fillText("?", 86, 26 + Math.sin(t / 300) * 2);
        break;
      }
      case "exclaim": {
        const pop = 1 + Math.abs(Math.sin(t / 250)) * 0.15;
        ctx.translate(86, 24);
        ctx.scale(pop, pop);
        ctx.fillStyle = "#ff9f0a";
        ctx.font = "bold 20px sans-serif";
        ctx.fillText("!", 0, 0);
        break;
      }
      case "anger": {
        // The "popping vein" mark.
        const pulse = 1 + Math.abs(Math.sin(t / 200)) * 0.2;
        ctx.translate(84, 18);
        ctx.scale(pulse, pulse);
        ctx.strokeStyle = ANGER;
        ctx.lineWidth = 2.2;
        for (let i = 0; i < 4; i++) {
          ctx.rotate(Math.PI / 2);
          ctx.beginPath();
          ctx.arc(5, 5, 3.5, Math.PI, Math.PI * 1.5);
          ctx.stroke();
        }
        break;
      }
      case "sparkle":
        ctx.fillStyle = SPARK;
        for (const [x, y, phase] of [
          [14, 22, 0],
          [87, 16, 1.3],
          [89, 48, 2.6],
        ] as const) {
          const s = 2 + Math.abs(Math.sin(t / 300 + phase)) * 3;
          drawStar(ctx, x, y, s);
        }
        break;
      case "thought":
        ctx.fillStyle = "rgba(190, 190, 205, 0.9)";
        for (const [x, y, r, i] of [
          [70, 30, 1.6, 0],
          [78, 21, 2.6, 1],
          [88, 10, 4, 2],
        ] as const) {
          ctx.globalAlpha = (t / 400) % 4 > i ? 1 : 0.25;
          ctx.beginPath();
          ctx.arc(x, y, r, 0, Math.PI * 2);
          ctx.fill();
        }
        break;
      case "confetti":
        for (let i = 0; i < 8; i++) {
          const x = 8 + i * 12 + Math.sin(t / 300 + i) * 3;
          const y = ((t / 18 + i * 37) % 60) - 6;
          ctx.save();
          ctx.translate(x, y);
          ctx.rotate(t / 200 + i);
          ctx.fillStyle = CONFETTI[i % CONFETTI.length] ?? ACCENT;
          ctx.globalAlpha = 1 - y / 70;
          ctx.fillRect(-2, -1, 4, 2.2);
          ctx.restore();
        }
        break;
      case "tears":
      case "sweat":
        // Drawn on the face.
        break;
    }
    ctx.restore();
  }
  ctx.restore();
}

/** A four-point twinkle. */
function drawStar(ctx: CanvasRenderingContext2D, x: number, y: number, s: number) {
  ctx.beginPath();
  ctx.moveTo(x, y - s);
  ctx.quadraticCurveTo(x, y, x + s, y);
  ctx.quadraticCurveTo(x, y, x, y + s);
  ctx.quadraticCurveTo(x, y, x - s, y);
  ctx.quadraticCurveTo(x, y, x, y - s);
  ctx.fill();
}
