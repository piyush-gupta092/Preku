import { describe, expect, it } from "vitest";
import { BEAT_MS, BLEND_MS, DANCE_MOVES, MOVE_MS, currentMove, danceAt, danceFrame } from "./dance";

describe("Pip dances", () => {
  it("rotates through every move", () => {
    const seen = new Set(DANCE_MOVES.map((_, i) => currentMove(i * MOVE_MS + 10)));
    expect(seen.size).toBe(DANCE_MOVES.length);
    expect(currentMove(DANCE_MOVES.length * MOVE_MS)).toBe(DANCE_MOVES[0]);
  });

  it("keeps every move inside the drawing box", () => {
    for (const move of DANCE_MOVES) {
      for (let t = 0; t < MOVE_MS; t += 37) {
        const f = danceFrame(move, t);
        expect(Math.abs(f.x), `${move} x`).toBeLessThanOrEqual(8);
        expect(f.y, `${move} y`).toBeGreaterThanOrEqual(0);
        expect(f.y, `${move} y`).toBeLessThanOrEqual(6);
        expect(Math.abs(f.tilt), `${move} tilt`).toBeLessThanOrEqual(0.2);
        for (const arm of [f.armL, f.armR]) {
          expect(arm, `${move} arm`).toBeGreaterThanOrEqual(0);
          expect(arm, `${move} arm`).toBeLessThanOrEqual(Math.PI);
        }
        for (const foot of [f.footL, f.footR]) {
          expect(foot.lift, `${move} foot`).toBeGreaterThanOrEqual(0);
          expect(foot.lift, `${move} foot`).toBeLessThanOrEqual(8);
        }
      }
    }
  });

  it("moves actually move", () => {
    for (const move of DANCE_MOVES) {
      const a = danceFrame(move, 0);
      const b = danceFrame(move, BEAT_MS * 1.25);
      expect(a, move).not.toEqual(b);
    }
  });

  it("never jumps between frames (except the robot, which snaps on purpose)", () => {
    for (const move of DANCE_MOVES) {
      if (move === "robot") continue;
      let prev = danceFrame(move, 0);
      for (let t = 16; t < MOVE_MS; t += 16) {
        const f = danceFrame(move, t);
        const at = `${move} at ${t} ms`;
        expect(Math.abs(f.x - prev.x), `${at} x`).toBeLessThan(0.6);
        expect(Math.abs(f.y - prev.y), `${at} y`).toBeLessThan(1);
        expect(Math.abs(f.tilt - prev.tilt), `${at} tilt`).toBeLessThan(0.05);
        expect(Math.abs(f.armL - prev.armL), `${at} armL`).toBeLessThan(0.35);
        expect(Math.abs(f.armR - prev.armR), `${at} armR`).toBeLessThan(0.35);
        for (const side of ["footL", "footR"] as const) {
          expect(Math.abs(f[side].dx - prev[side].dx), `${at} ${side} dx`).toBeLessThan(1);
          // Naatu steps four times a beat, so its feet move fast (but never teleport).
          expect(Math.abs(f[side].lift - prev[side].lift), `${at} ${side} lift`).toBeLessThan(2);
        }
        prev = f;
      }
    }
  });

  it("moonwalk glides there and back without jumping back to the start", () => {
    let wentLeft = false;
    let cameBack = false;
    let previous = danceFrame("moonwalk", 0).x;
    for (let t = 16; t < MOVE_MS; t += 16) {
      const x = danceFrame("moonwalk", t).x;
      expect(Math.abs(x - previous), `jump at ${t} ms`).toBeLessThan(0.2);
      if (x < previous) wentLeft = true;
      if (wentLeft && x > previous) cameBack = true;
      previous = x;
    }
    expect(wentLeft && cameBack).toBe(true);
    // Starts and ends centred, so the moves before and after line up.
    expect(danceFrame("moonwalk", 0).x).toBeCloseTo(0);
    expect(danceFrame("moonwalk", MOVE_MS - 1).x).toBeCloseTo(0, 1);
  });

  it("eases from one move into the next instead of jumping", () => {
    for (let i = 1; i <= DANCE_MOVES.length; i++) {
      const boundary = i * MOVE_MS;
      const before = currentMove(boundary - 1);
      const after = currentMove(boundary + 1);
      // The robot snaps on purpose, so skip changes involving it.
      if (before === "robot" || after === "robot") continue;
      const a = danceAt(boundary - 8);
      const b = danceAt(boundary + 8);
      expect(Math.abs(a.x - b.x), `${before} → ${after} x`).toBeLessThan(1);
      expect(Math.abs(a.armL - b.armL), `${before} → ${after} arm`).toBeLessThan(0.3);
      expect(Math.abs(a.armR - b.armR), `${before} → ${after} arm`).toBeLessThan(0.3);
    }
    // Fully on the new move once the blend is over.
    expect(danceAt(MOVE_MS + BLEND_MS + 1)).toEqual(
      danceFrame(currentMove(MOVE_MS + 1), MOVE_MS + BLEND_MS + 1),
    );
  });

  it("bhangra keeps both arms up, robot snaps", () => {
    const b = danceFrame("bhangra", 123);
    expect(b.armL).toBeGreaterThan(2.5);
    expect(b.armR).toBeGreaterThan(2.5);
    // Robot holds a pose for a whole half beat.
    expect(danceFrame("robot", 10)).toEqual(danceFrame("robot", BEAT_MS / 2 - 10));
  });
});
