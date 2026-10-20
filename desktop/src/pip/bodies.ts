// Pip's three bodies (Design.md §6, concepts in design/mascot-concepts.html). Units: the ground
// point is (0, 0), up is negative y, and a full figure is about 220 units tall. Each body says
// where its joints are and draws its own torso/head; the renderer adds limbs, face and effects.

import type { MascotStyle } from "../lib/types/generated/MascotStyle";
import type { PipPalette } from "./palette";

export interface Point {
  x: number;
  y: number;
}

export interface Body {
  /** Right shoulder and hip; the left ones mirror them. */
  shoulder: Point;
  hip: Point;
  arm: [number, number];
  armWidth: number;
  leg: [number, number];
  legWidth: number;
  handRadius: number;
  /** Shoe ellipse radii. */
  foot: [number, number];
  /** Eye-line centre and how much the shared face is scaled. */
  face: Point & { scale: number };
  /** Where the leaves sprout, and their size. */
  leaves: Point & { scale: number };
  /** Head centre and radius, for placing floating effects (?, !, hearts…). */
  head: Point & { r: number };
  /** The head-only crop used at small sizes: vertical centre and height. */
  bust: { cy: number; h: number };
  /** Classic Pip is soft-edged: no dark outlines. */
  outline?: boolean;
  /** Small sizes still draw the whole figure (Classic is all head anyway). */
  wholeInBust?: boolean;
  /** Torso, drawn behind the head (and in front of hanging arms). */
  drawTorso(ctx: CanvasRenderingContext2D, palette: PipPalette): void;
  /** Head (or the ghost's sheet). */
  drawHead(ctx: CanvasRenderingContext2D, palette: PipPalette): void;
}

export const OUTLINE = 4;

function outlined(ctx: CanvasRenderingContext2D, palette: PipPalette, fill: string) {
  ctx.fillStyle = fill;
  ctx.fill();
  ctx.lineWidth = OUTLINE;
  ctx.lineJoin = "round";
  ctx.strokeStyle = palette.outline;
  ctx.stroke();
}

/** A round head with a soft shine and a darker chin. */
function roundHead(
  ctx: CanvasRenderingContext2D,
  palette: PipPalette,
  cx: number,
  cy: number,
  rx: number,
  ry: number,
) {
  ctx.beginPath();
  ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
  ctx.fillStyle = palette.body;
  ctx.fill();
  ctx.save();
  ctx.clip();
  ctx.fillStyle = palette.shade;
  ctx.beginPath();
  ctx.ellipse(cx, cy + ry * 0.85, rx * 0.95, ry * 0.45, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
  ctx.beginPath();
  ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
  ctx.lineWidth = OUTLINE;
  ctx.strokeStyle = palette.outline;
  ctx.stroke();
  shine(ctx, cx - rx * 0.45, cy - ry * 0.55, rx * 0.22, ry * 0.12);
}

function shine(ctx: CanvasRenderingContext2D, x: number, y: number, rx: number, ry: number) {
  ctx.save();
  ctx.globalAlpha = 0.45;
  ctx.fillStyle = "#ffffff";
  ctx.beginPath();
  ctx.ellipse(x, y, rx, ry, -0.45, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

/**
 * The original Pip (Design.md §6) at 2.5× its old 100-unit size: a round pebble whose body is
 * its head, a small face, stubby arms that peek out past its sides and little feet.
 */
const CLASSIC: Body = {
  shoulder: { x: 66, y: -96 },
  hip: { x: 30, y: -24 },
  arm: [20, 17],
  armWidth: 17,
  leg: [9, 8],
  legWidth: 20,
  handRadius: 9,
  foot: [20, 11],
  face: { x: 0, y: -110, scale: 2.5 },
  leaves: { x: 0, y: -168, scale: 1.25 },
  head: { x: 0, y: -100, r: 78 },
  bust: { cy: -104, h: 236 },
  outline: false,
  wholeInBust: true,
  drawTorso() {
    // The body is the head.
  },
  drawHead(ctx, palette) {
    ctx.beginPath();
    ctx.ellipse(0, -100, 90, 78, 0, 0, Math.PI * 2);
    ctx.fillStyle = palette.body;
    ctx.fill();
    ctx.save();
    ctx.clip();
    ctx.fillStyle = palette.shade;
    ctx.beginPath();
    ctx.ellipse(0, -37, 80, 40, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  },
};

const SPROUT: Body = {
  shoulder: { x: 17, y: -100 },
  hip: { x: 9, y: -66 },
  arm: [27, 25],
  armWidth: 9,
  leg: [30, 29],
  legWidth: 10,
  handRadius: 8,
  foot: [13, 8],
  face: { x: 0, y: -144, scale: 1.3 },
  leaves: { x: 0, y: -190, scale: 1 },
  head: { x: 0, y: -150, r: 45 },
  bust: { cy: -166, h: 128 },
  drawTorso(ctx, palette) {
    ctx.beginPath();
    ctx.ellipse(0, -84, 21, 25, 0, 0, Math.PI * 2);
    outlined(ctx, palette, palette.body);
  },
  drawHead(ctx, palette) {
    roundHead(ctx, palette, 0, -150, 47, 43);
  },
};

const GHOST: Body = {
  shoulder: { x: 44, y: -106 },
  hip: { x: 14, y: -58 },
  arm: [21, 19],
  armWidth: 9,
  leg: [26, 25],
  legWidth: 10,
  handRadius: 8,
  foot: [12, 7],
  face: { x: 0, y: -112, scale: 1.25 },
  leaves: { x: 0, y: -131, scale: 0.9 },
  head: { x: 0, y: -110, r: 50 },
  bust: { cy: -112, h: 116 },
  drawTorso() {
    // The sheet is the whole body.
  },
  drawHead(ctx, palette) {
    const sheet = () => {
      ctx.beginPath();
      ctx.moveTo(-48, -72);
      ctx.bezierCurveTo(-54, -152, 54, -152, 48, -72);
      // Wavy hem, right to left.
      for (let i = 0; i < 6; i++) {
        const x = 48 - i * 16;
        ctx.quadraticCurveTo(x - 8, -60, x - 16, i === 5 ? -72 : -70);
      }
      ctx.closePath();
    };
    sheet();
    ctx.fillStyle = palette.body;
    ctx.fill();
    ctx.save();
    ctx.clip();
    // Folds: a light one on the left, a shaded one on the right.
    ctx.fillStyle = palette.shade;
    ctx.beginPath();
    ctx.ellipse(30, -76, 34, 26, -0.3, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
    sheet();
    ctx.lineWidth = OUTLINE;
    ctx.lineJoin = "round";
    ctx.strokeStyle = palette.outline;
    ctx.stroke();
    ctx.save();
    ctx.globalAlpha = 0.5;
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 5;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(-36, -82);
    ctx.bezierCurveTo(-38, -108, -30, -124, -18, -132);
    ctx.stroke();
    ctx.restore();
  },
};

const CHUBBY: Body = {
  shoulder: { x: 30, y: -86 },
  hip: { x: 17, y: -40 },
  arm: [18, 16],
  armWidth: 14,
  leg: [16, 15],
  legWidth: 16,
  handRadius: 9.5,
  foot: [15, 9],
  face: { x: 0, y: -134, scale: 1.35 },
  leaves: { x: 0, y: -181, scale: 1 },
  head: { x: 0, y: -140, r: 48 },
  bust: { cy: -156, h: 128 },
  drawTorso(ctx, palette) {
    ctx.beginPath();
    ctx.ellipse(0, -72, 40, 36, 0, 0, Math.PI * 2);
    outlined(ctx, palette, palette.body);
    ctx.fillStyle = palette.belly;
    ctx.beginPath();
    ctx.ellipse(0, -64, 22, 20, 0, 0, Math.PI * 2);
    ctx.fill();
  },
  drawHead(ctx, palette) {
    roundHead(ctx, palette, 0, -140, 50, 44);
  },
};

export const BODIES: Record<MascotStyle, Body> = {
  classic: CLASSIC,
  sprout: SPROUT,
  ghost: GHOST,
  chubby: CHUBBY,
};

export const MASCOT_STYLES: MascotStyle[] = ["classic", "sprout", "ghost", "chubby"];

export function bodyFor(style: MascotStyle | undefined): Body {
  return BODIES[style ?? "classic"] ?? CLASSIC;
}

/**
 * Two-bone inverse kinematics: the elbow/knee for a limb from `start` reaching `target`,
 * bending outwards (towards `side`). Targets out of reach are clamped.
 */
export function solveLimb(
  start: Point,
  target: Point,
  lengths: [number, number],
  side: -1 | 1,
): { mid: Point; end: Point } {
  const [a, b] = lengths;
  const dx = target.x - start.x;
  const dy = target.y - start.y;
  const reach = Math.hypot(dx, dy);
  const d = Math.min(Math.max(reach, Math.abs(a - b) + 0.01), a + b - 0.01);
  const base = Math.atan2(dy, dx);
  const offset = Math.acos(Math.min(1, Math.max(-1, (a * a + d * d - b * b) / (2 * a * d))));
  const candidates = [base + offset, base - offset].map((angle) => ({
    x: start.x + Math.cos(angle) * a,
    y: start.y + Math.sin(angle) * a,
  }));
  const mid = candidates[0]!.x * side >= candidates[1]!.x * side ? candidates[0]! : candidates[1]!;
  const end = {
    x: start.x + (dx / (reach || 1)) * d,
    y: start.y + (dy / (reach || 1)) * d,
  };
  return { mid, end };
}

/**
 * Forward kinematics for an arm from one "raise" angle (0 = hanging, π = straight up, the same
 * scale expressions and dances use). Raised arms go out first, then up at the elbow, so the
 * hand clears the head.
 */
export function armFromAngle(
  start: Point,
  angle: number,
  lengths: [number, number],
  side: -1 | 1,
): { mid: Point; end: Point; upper: number } {
  const upper = angle <= 1.5 ? angle * 0.75 : 1.125 + (angle - 1.5) * 0.6;
  const lower = Math.min(angle + (angle > 1.5 ? 0.35 : 0.2), 3.1);
  const mid = {
    x: start.x + side * Math.sin(upper) * lengths[0],
    y: start.y + Math.cos(upper) * lengths[0],
  };
  const end = {
    x: mid.x + side * Math.sin(lower) * lengths[1],
    y: mid.y + Math.cos(lower) * lengths[1],
  };
  return { mid, end, upper };
}
