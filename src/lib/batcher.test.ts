import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createBatcher } from "./batcher";

describe("createBatcher", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("delivers many items in one flush per interval", () => {
    const flushes: number[][] = [];
    const b = createBatcher<number>(50, (items) => flushes.push(items));
    for (let i = 0; i < 100; i++) b.push(i);
    expect(flushes).toHaveLength(0);
    vi.advanceTimersByTime(50);
    expect(flushes).toHaveLength(1);
    expect(flushes[0]).toHaveLength(100);
  });

  it("bounds flushes to one per 50 ms under a steady stream", () => {
    let flushes = 0;
    const b = createBatcher<number>(50, () => flushes++);
    for (let t = 0; t < 1000; t += 5) {
      b.push(t);
      vi.advanceTimersByTime(5);
    }
    expect(flushes).toBeLessThanOrEqual(20);
    expect(flushes).toBeGreaterThanOrEqual(19);
  });

  it("flushNow delivers immediately; dispose drops pending items", () => {
    const seen: number[][] = [];
    const b = createBatcher<number>(50, (i) => seen.push(i));
    b.push(1);
    b.flushNow();
    expect(seen).toEqual([[1]]);
    b.push(2);
    b.dispose();
    vi.advanceTimersByTime(100);
    expect(seen).toEqual([[1]]);
  });
});
