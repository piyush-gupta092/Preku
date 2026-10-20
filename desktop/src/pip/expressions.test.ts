import { describe, expect, it } from "vitest";
import { en } from "../lib/i18n/en";
import { EXPRESSIONS, GALLERY } from "./expressions";
import { MASCOT_STYLES } from "./bodies";
import { paletteFor } from "./palette";
import { drawPip, type PipMood } from "./renderer";

/** A canvas context that accepts every call and remembers where text and shapes went. */
function fakeContext(): CanvasRenderingContext2D {
  return new Proxy({} as Record<string, unknown>, {
    get: (target, key) => (key in target ? target[key as string] : () => undefined),
    set: (target, key, value) => {
      target[key as string] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

const palette = paletteFor("#ffb38a");

describe("Pip expressions", () => {
  it("lists the 20 expressions, each with a recipe and words in Settings", () => {
    expect(GALLERY).toHaveLength(20);
    expect(new Set(GALLERY).size).toBe(20);
    for (const id of GALLERY) {
      expect(EXPRESSIONS[id]).toBeDefined();
      expect(en.settings.expressions[id]?.name).toBeTruthy();
      expect(en.settings.expressions[id]?.look).toBeTruthy();
    }
  });

  it("keeps arms between hanging down and straight up", () => {
    for (const [mood, spec] of Object.entries(EXPRESSIONS)) {
      for (let t = 0; t < 4000; t += 137) {
        for (const side of [-1, 1] as const) {
          const angle = spec.arms(t, side);
          expect(angle, `${mood} arm`).toBeGreaterThanOrEqual(0);
          expect(angle, `${mood} arm`).toBeLessThanOrEqual(Math.PI);
        }
      }
    }
  });

  it("keeps hops and tilts small enough to stay inside the canvas", () => {
    for (const spec of Object.values(EXPRESSIONS)) {
      for (let t = 0; t < 4000; t += 97) {
        const hop = spec.bob?.(t) ?? 0;
        expect(hop).toBeGreaterThanOrEqual(0);
        expect(hop).toBeLessThanOrEqual(1);
        expect(Math.abs(spec.tilt?.(t) ?? 0)).toBeLessThan(0.25);
        expect(Math.abs(spec.shake?.(t) ?? 0)).toBeLessThanOrEqual(2);
      }
    }
  });

  it("draws every mood in every body at every size without throwing", () => {
    for (const style of MASCOT_STYLES)
      for (const mood of Object.keys(EXPRESSIONS) as PipMood[]) {
        for (const size of [24, 56, 120]) {
          for (const still of [false, true]) {
            expect(() =>
              drawPip(
                fakeContext(),
                size,
                {
                  mood,
                  breathe: 1,
                  squish: 1,
                  hop: 0,
                  eyeOpen: 1,
                  lookX: 0,
                  lookY: 0,
                  time: 1234,
                  still,
                },
                palette,
                style,
              ),
            ).not.toThrow();
          }
        }
      }
  });
});
