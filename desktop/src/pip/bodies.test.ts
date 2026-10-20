import { describe, expect, it } from "vitest";
import { armFromAngle, BODIES, MASCOT_STYLES, solveLimb } from "./bodies";
import { paletteFor } from "./palette";

const dist = (a: { x: number; y: number }, b: { x: number; y: number }) =>
  Math.hypot(a.x - b.x, a.y - b.y);

describe("Pip bodies", () => {
  it("has the four styles the user can pick", () => {
    expect(MASCOT_STYLES).toEqual(["classic", "sprout", "ghost", "chubby"]);
    for (const style of MASCOT_STYLES) expect(BODIES[style]).toBeDefined();
  });

  it("IK keeps limb lengths and bends the joint outwards", () => {
    const start = { x: 10, y: -60 };
    for (const side of [-1, 1] as const) {
      const target = { x: side * 14, y: -10 };
      const { mid, end } = solveLimb(start, target, [30, 29], side);
      expect(dist(start, mid)).toBeCloseTo(30, 5);
      expect(dist(mid, end)).toBeCloseTo(29, 5);
      // The knee sits further out than the straight line between hip and foot.
      expect(mid.x * side).toBeGreaterThan(((start.x + end.x) / 2) * side);
    }
  });

  it("IK stops short of targets out of reach", () => {
    const start = { x: 0, y: 0 };
    const { end } = solveLimb(start, { x: 0, y: 500 }, [30, 29], 1);
    expect(dist(start, end)).toBeLessThan(59);
    expect(end.y).toBeGreaterThan(55);
  });

  it("raised arms reach above the shoulder and out to the side", () => {
    for (const style of MASCOT_STYLES) {
      const body = BODIES[style];
      const start = { x: body.shoulder.x, y: body.shoulder.y };
      const up = armFromAngle(start, 2.8, body.arm, 1);
      expect(up.end.y, style).toBeLessThan(start.y);
      expect(up.mid.x, style).toBeGreaterThan(start.x);
      const down = armFromAngle(start, 0.3, body.arm, 1);
      expect(down.end.y, style).toBeGreaterThan(start.y);
    }
  });

  it("derives a full palette from any body color", () => {
    const p = paletteFor("#9cc8ff");
    expect(p.body).toBe("rgb(156, 200, 255)");
    expect(p.outline).not.toBe(p.body);
    // Garbage falls back to the default peach.
    expect(paletteFor("nope").body).toBe(paletteFor("#ffb38a").body);
  });
});
