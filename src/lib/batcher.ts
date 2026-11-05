/**
 * Collects items and hands them over at most once per `intervalMs`, so fast
 * event streams (tokens, partial transcript text) don't re-render on every
 * item (tech doc 13.5).
 */
export function createBatcher<T>(intervalMs: number, flush: (items: T[]) => void) {
  let pending: T[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;

  const run = () => {
    timer = null;
    const items = pending;
    pending = [];
    if (items.length) flush(items);
  };

  return {
    push(item: T) {
      pending.push(item);
      timer ??= setTimeout(run, intervalMs);
    },
    /** Delivers anything pending now (e.g. before unmount). */
    flushNow() {
      if (timer) clearTimeout(timer);
      run();
    },
    dispose() {
      if (timer) clearTimeout(timer);
      timer = null;
      pending = [];
    },
  };
}
