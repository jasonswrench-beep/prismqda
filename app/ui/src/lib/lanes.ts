// Gives each margin stripe a column so that stripes that cover the same lines sit side by side.

export interface StripeBox {
  id: number;
  /** Vertical extent in pixels. */
  top: number;
  bottom: number;
}

/**
 * Greedy interval partitioning: boxes are placed in order of top, each in the first lane whose last
 * box ended above it. Boxes that touch within `gap` pixels count as overlapping.
 */
export function assignLanes(boxes: StripeBox[], maxLanes = 6, gap = 1): { lanes: Map<number, number>; count: number; overflow: number[] } {
  const sorted = [...boxes].sort((a, b) => a.top - b.top || b.bottom - a.bottom || a.id - b.id);
  const laneEnds: number[] = [];
  const lanes = new Map<number, number>();
  const overflow: number[] = [];
  for (const box of sorted) {
    let lane = laneEnds.findIndex((end) => end + gap <= box.top);
    if (lane === -1) {
      if (laneEnds.length >= maxLanes) {
        // Too many at once: share the last lane and remember who did not get their own.
        lane = maxLanes - 1;
        overflow.push(box.id);
        laneEnds[lane] = Math.max(laneEnds[lane], box.bottom);
        lanes.set(box.id, lane);
        continue;
      }
      lane = laneEnds.length;
      laneEnds.push(box.bottom);
    } else {
      laneEnds[lane] = box.bottom;
    }
    lanes.set(box.id, lane);
  }
  return { lanes, count: laneEnds.length, overflow };
}
