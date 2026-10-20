import { describe, expect, it } from "vitest";
import { PipAnimator, blink, breathe, squish } from "./animator";

describe("animation curves", () => {
  it("blink closes the eye halfway through and reopens", () => {
    expect(blink(-1)).toBe(1);
    expect(blink(70)).toBeCloseTo(0);
    expect(blink(140)).toBe(1);
  });

  it("squish dips to 0.85, overshoots to 1.05 and settles", () => {
    expect(squish(0)).toBe(1);
    expect(squish(98)).toBeCloseTo(0.85);
    expect(squish(196)).toBeCloseTo(1.05);
    expect(squish(280)).toBe(1);
  });

  it("breathing stays within 1.0–1.03", () => {
    for (let t = 0; t < 3200; t += 100) {
      expect(breathe(t)).toBeGreaterThanOrEqual(1);
      expect(breathe(t)).toBeLessThanOrEqual(1.03 + 1e-9);
    }
  });
});

describe("PipAnimator", () => {
  it("blinks after the random gap", () => {
    const animator = new PipAnimator(0, () => 0); // gap = 3000 ms
    expect(animator.pose(2999, "idle", false).eyeOpen).toBe(1);
    animator.pose(3000, "idle", false); // blink starts
    expect(animator.pose(3070, "idle", false).eyeOpen).toBeCloseTo(0);
  });

  it("gets dizzy after 5 quick pokes, then recovers", () => {
    const animator = new PipAnimator(0, () => 0.5);
    for (let i = 0; i < 5; i++) animator.poke(i * 100);
    expect(animator.pose(500, "idle", false).mood).toBe("dizzy");
    expect(animator.pose(400 + 1500, "idle", false).mood).toBe("idle");
  });

  it("slow pokes never make Pip dizzy", () => {
    const animator = new PipAnimator(0, () => 0.5);
    for (let i = 0; i < 10; i++) animator.poke(i * 1000);
    expect(animator.isDizzy(9001)).toBe(false);
  });

  it("reduced motion disables breathing, squish and hops", () => {
    const animator = new PipAnimator(0, () => 0.5);
    animator.poke(0);
    const pose = animator.pose(100, "celebrate", true);
    expect(pose.breathe).toBe(1);
    expect(pose.squish).toBe(1);
    expect(pose.hop).toBe(0);
  });

  it("dances with a frame, or stays a still happy Pip with reduced motion", () => {
    const animator = new PipAnimator(0, () => 0.5);
    const dancing = animator.pose(1234, "dance", false);
    expect(dancing.mood).toBe("dance");
    expect(dancing.dance).toBeDefined();
    expect(dancing.hop).toBe(0);
    const calm = animator.pose(1234, "dance", true);
    expect(calm.mood).toBe("happy");
    expect(calm.dance).toBeUndefined();
  });

  it("clamps look direction", () => {
    const animator = new PipAnimator(0);
    animator.look(5, -5);
    const pose = animator.pose(0, "idle", false);
    expect(pose.lookX).toBe(1);
    expect(pose.lookY).toBe(-1);
  });
});
