/** Drag further than this (px) for a push/pull to count. */
export const DRAG_THRESHOLD = 24;
/** Movement below this (px) is treated as a plain click. */
const CLICK_SLOP = 4;
/** Accumulated wheel delta needed to open/close (one notch is ~100 on most mice). */
export const WHEEL_THRESHOLD = 40;

export type HandleGesture = "hide" | "cancel";

/**
 * Releasing the bottom handle: a click or an upward push hides the island; dragging it
 * down (or a half-hearted push) snaps back.
 */
export function classifyHandleRelease(dy: number): HandleGesture {
  if (Math.abs(dy) < CLICK_SLOP) return "hide";
  return dy <= -DRAG_THRESHOLD ? "hide" : "cancel";
}

/** Pulling the compact pill down far enough opens the panel. */
export function isPullDown(dy: number): boolean {
  return dy >= DRAG_THRESHOLD;
}

/** Visual follow while dragging: moves up freely, resists downward pulls. */
export function dragOffset(dy: number): number {
  return dy < 0 ? dy : Math.min(dy * 0.25, 12);
}

/** The bits of an element needed to tell whether it can still scroll. */
export interface ScrollBox {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  parentElement: ScrollBox | null;
}

/**
 * True when some scrollable element between `start` and `stop` can still scroll in the wheel's
 * direction — then the wheel scrolls that list instead of opening/closing the island.
 */
export function canScrollWithin(start: ScrollBox | null, stop: unknown, deltaY: number): boolean {
  for (let el = start; el && el !== stop; el = el.parentElement) {
    if (el.scrollHeight <= el.clientHeight + 1) continue;
    const canMove =
      deltaY < 0 ? el.scrollTop > 0 : el.scrollTop + el.clientHeight < el.scrollHeight - 1;
    if (canMove) return true;
  }
  return false;
}
