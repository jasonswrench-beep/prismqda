// Thread view: units that reply to other units are listed under their parents, indented.

import type { UnitRow } from '../types';

export interface ThreadRow {
  unit: UnitRow;
  /** 0 for a post that replies to nothing. */
  depth: number;
  /** Position in the plain text order, for "unit 12 of 40". */
  index: number;
}

/**
 * Depth-first order: each unit is followed by its replies in text order. A unit whose parent is
 * missing from the list, or that is part of a loop, is treated as having no parent.
 */
export function threadRows(units: readonly UnitRow[]): ThreadRow[] {
  const byId = new Map<number, UnitRow>();
  const order = new Map<number, number>();
  units.forEach((u, i) => {
    byId.set(u.id, u);
    order.set(u.id, i);
  });
  const children = new Map<number, UnitRow[]>();
  const roots: UnitRow[] = [];
  for (const u of units) {
    const parent = u.parent_id !== null && u.parent_id !== u.id && byId.has(u.parent_id) ? u.parent_id : null;
    if (parent === null) roots.push(u);
    else {
      const list = children.get(parent);
      if (list) list.push(u);
      else children.set(parent, [u]);
    }
  }
  const out: ThreadRow[] = [];
  const seen = new Set<number>();
  // Iterative so a deep thread of thousands of replies cannot overflow the call stack.
  const stack: { unit: UnitRow; depth: number }[] = [];
  const pushAll = (list: readonly UnitRow[], depth: number) => {
    for (let i = list.length - 1; i >= 0; i--) stack.push({ unit: list[i], depth });
  };
  pushAll(roots, 0);
  while (stack.length) {
    const { unit, depth } = stack.pop()!;
    if (seen.has(unit.id)) continue;
    seen.add(unit.id);
    out.push({ unit, depth, index: order.get(unit.id) ?? 0 });
    const kids = children.get(unit.id);
    if (kids) pushAll(kids, depth + 1);
  }
  // Anything left is in a loop with no way in; list it at the top level so nothing is lost.
  for (const u of units) {
    if (!seen.has(u.id)) {
      seen.add(u.id);
      out.push({ unit: u, depth: 0, index: order.get(u.id) ?? 0 });
    }
  }
  return out;
}

/** True when at least one unit replies to another. */
export function hasReplies(units: readonly UnitRow[]): boolean {
  return units.some((u) => u.parent_id !== null);
}
