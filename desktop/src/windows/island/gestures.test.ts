import { describe, expect, it } from "vitest";
import {
  canScrollWithin,
  classifyHandleRelease,
  dragOffset,
  isPullDown,
  type ScrollBox,
} from "./gestures";

describe("compact pill", () => {
  it("opens when pulled down far enough", () => {
    expect(isPullDown(24)).toBe(true);
    expect(isPullDown(10)).toBe(false);
    expect(isPullDown(-40)).toBe(false);
  });
});

describe("bottom handle", () => {
  it("a click hides", () => {
    expect(classifyHandleRelease(0)).toBe("hide");
    expect(classifyHandleRelease(2)).toBe("hide");
  });

  it("pushing up far enough hides", () => {
    expect(classifyHandleRelease(-24)).toBe("hide");
    expect(classifyHandleRelease(-120)).toBe("hide");
  });

  it("a short push or a downward drag snaps back", () => {
    expect(classifyHandleRelease(-10)).toBe("cancel");
    expect(classifyHandleRelease(40)).toBe("cancel");
  });

  it("follows upward drags and resists downward ones", () => {
    expect(dragOffset(-50)).toBe(-50);
    expect(dragOffset(20)).toBe(5);
    expect(dragOffset(400)).toBe(12);
  });
});

describe("wheel inside scrollable lists", () => {
  const box = (scrollTop: number, parent: ScrollBox | null = null): ScrollBox => ({
    scrollTop,
    scrollHeight: 500,
    clientHeight: 200,
    parentElement: parent,
  });
  const plain = (parent: ScrollBox | null): ScrollBox => ({
    scrollTop: 0,
    scrollHeight: 20,
    clientHeight: 20,
    parentElement: parent,
  });

  it("lets a list scroll up before the panel closes", () => {
    const list = box(120);
    expect(canScrollWithin(plain(list), null, -50)).toBe(true);
    expect(canScrollWithin(plain(box(0)), null, -50)).toBe(false);
  });

  it("lets a list scroll down until its end", () => {
    expect(canScrollWithin(box(0), null, 50)).toBe(true);
    expect(canScrollWithin(box(300), null, 50)).toBe(false);
  });

  it("stops at the island root", () => {
    const root = box(100);
    expect(canScrollWithin(plain(root), root, -50)).toBe(false);
  });
});
