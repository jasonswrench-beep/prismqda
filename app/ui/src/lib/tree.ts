// The codebook as a tree: building it from the flat list the engine returns, filtering it, and
// working out where a code lands when it is moved up, down, in, or out.

import type { CodeRow } from '../types';
import { t } from '../i18n';

export interface CodeNode {
  code: CodeRow;
  children: CodeNode[];
  /** True when the code is hidden by default but shown because an active code sits beneath it. */
  onlyAsParent: boolean;
}

function asciiLower(s: string): string {
  // SQLite's lower() changes only A to Z, and the engine orders siblings with it.
  return s.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

function compareText(a: string, b: string): number {
  // Code point order, which is what SQLite does with UTF-8 text (JavaScript compares UTF-16 units).
  const x = Array.from(a);
  const y = Array.from(b);
  const n = Math.min(x.length, y.length);
  for (let i = 0; i < n; i++) {
    const d = x[i].codePointAt(0)! - y[i].codePointAt(0)!;
    if (d !== 0) return d;
  }
  return x.length - y.length;
}

/** The order the engine keeps siblings in: sort_order, then name without regard to case, then id. */
export function compareCodes(a: Pick<CodeRow, 'sort_order' | 'name' | 'id'>, b: Pick<CodeRow, 'sort_order' | 'name' | 'id'>): number {
  return a.sort_order - b.sort_order || compareText(asciiLower(a.name), asciiLower(b.name)) || a.id - b.id;
}

/** All codes (any status) that share a parent, in sibling order. */
export function childrenOf(codes: readonly CodeRow[], parentId: number | null): CodeRow[] {
  return codes.filter((c) => c.parent_id === parentId).sort(compareCodes);
}

export function buildCodeTree(codes: readonly CodeRow[], opts: { showInactive: boolean }): CodeNode[] {
  const byParent = new Map<number | null, CodeRow[]>();
  const ids = new Set(codes.map((c) => c.id));
  for (const c of codes) {
    // A code whose parent is missing is shown at the top rather than lost.
    const key = c.parent_id !== null && ids.has(c.parent_id) ? c.parent_id : null;
    const list = byParent.get(key);
    if (list) list.push(c);
    else byParent.set(key, [c]);
  }
  const seen = new Set<number>();
  const make = (parent: number | null): CodeNode[] => {
    const out: CodeNode[] = [];
    for (const code of (byParent.get(parent) ?? []).sort(compareCodes)) {
      if (seen.has(code.id)) continue;
      seen.add(code.id);
      const children = make(code.id);
      const inactive = code.status !== 'active';
      if (inactive && !opts.showInactive && children.length === 0) continue;
      out.push({ code, children, onlyAsParent: inactive && !opts.showInactive });
    }
    return out;
  };
  return make(null);
}

/** Every code in the tree in the order a person reads them, with its depth (0 for the top). */
export function flattenTree(nodes: readonly CodeNode[], depth = 0, out: { node: CodeNode; depth: number }[] = []): { node: CodeNode; depth: number }[] {
  for (const n of nodes) {
    out.push({ node: n, depth });
    flattenTree(n.children, depth + 1, out);
  }
  return out;
}

export interface FilteredTree {
  nodes: CodeNode[];
  /** Codes whose own name or label matched. */
  matches: Set<number>;
  /** Ids to show open so every match is visible. */
  expanded: Set<number>;
}

/** Keeps the codes that match the query (name or short label) and the parents that lead to them. */
export function filterCodeTree(nodes: readonly CodeNode[], query: string, labelOf: (id: number) => string = () => ''): FilteredTree {
  const q = query.trim().toLowerCase();
  if (q === '') return { nodes: [...nodes], matches: new Set(), expanded: new Set() };
  const matches = new Set<number>();
  const expanded = new Set<number>();
  const walk = (list: readonly CodeNode[]): CodeNode[] => {
    const out: CodeNode[] = [];
    for (const n of list) {
      const children = walk(n.children);
      const own = n.code.name.toLowerCase().includes(q) || labelOf(n.code.id).toLowerCase() === q;
      if (own) matches.add(n.code.id);
      if (own || children.length > 0) {
        if (children.length > 0) expanded.add(n.code.id);
        out.push({ ...n, children: own ? n.children : children });
      }
    }
    return out;
  };
  return { nodes: walk(nodes), matches, expanded };
}

export function descendantIds(codes: readonly CodeRow[], id: number): Set<number> {
  const out = new Set<number>();
  const stack = [id];
  while (stack.length) {
    const cur = stack.pop()!;
    for (const c of codes) {
      if (c.parent_id === cur && !out.has(c.id)) {
        out.add(c.id);
        stack.push(c.id);
      }
    }
  }
  return out;
}

export function ancestorIds(codes: readonly CodeRow[], id: number): number[] {
  const byId = new Map(codes.map((c) => [c.id, c]));
  const out: number[] = [];
  let cur = byId.get(id)?.parent_id ?? null;
  while (cur !== null && !out.includes(cur)) {
    out.push(cur);
    cur = byId.get(cur)?.parent_id ?? null;
  }
  return out;
}

/** The names from the top of the tree down to the code, such as "Support > Family". */
export function pathLabel(codes: readonly CodeRow[], id: number): string {
  const byId = new Map(codes.map((c) => [c.id, c]));
  const names: string[] = [];
  const chain = [id, ...ancestorIds(codes, id)];
  for (const cid of chain) {
    const c = byId.get(cid);
    if (c) names.push(c.name);
  }
  return names.reverse().join(' > ');
}

export type MoveKind = 'up' | 'down' | 'indent' | 'outdent';

export interface MovePlan {
  parent_id: number | null;
  /** Place among the new siblings with the moved code left out; omitted means last. */
  position?: number;
  /** Plain words for the announcement, such as "under Support". */
  where: string;
}

export type MoveResult = { ok: true; plan: MovePlan } | { ok: false; reason: string };

/**
 * Works out the arguments for `code.move` for the four keyboard moves. Hidden codes (retired or
 * merged, when not shown) are stepped over so the move matches what the person sees.
 */
export function planMove(codes: readonly CodeRow[], id: number, kind: MoveKind, showInactive: boolean): MoveResult {
  const me = codes.find((c) => c.id === id);
  if (!me) return { ok: false, reason: t('That code is not in the codebook any more.') };
  const visible = (c: CodeRow) => c.id === id || showInactive || c.status === 'active';
  const siblings = childrenOf(codes, me.parent_id);
  const mine = siblings.findIndex((c) => c.id === id);
  const othersAll = siblings.filter((c) => c.id !== id);

  if (kind === 'up' || kind === 'down') {
    if (kind === 'up') {
      const prev = [...siblings.slice(0, mine)].reverse().find(visible);
      if (!prev) return { ok: false, reason: t('{name} is already first.', { name: me.name }) };
      return { ok: true, plan: { parent_id: me.parent_id, position: othersAll.indexOf(prev), where: t('up one place') } };
    }
    const next = siblings.slice(mine + 1).find(visible);
    if (!next) return { ok: false, reason: t('{name} is already last.', { name: me.name }) };
    return { ok: true, plan: { parent_id: me.parent_id, position: othersAll.indexOf(next) + 1, where: t('down one place') } };
  }

  if (kind === 'indent') {
    const prev = [...siblings.slice(0, mine)].reverse().find(visible);
    if (!prev) return { ok: false, reason: t('There is no code above {name} to move it under.', { name: me.name }) };
    return { ok: true, plan: { parent_id: prev.id, where: t('under {name}', { name: prev.name }) } };
  }

  // outdent
  if (me.parent_id === null) return { ok: false, reason: t('{name} is already at the top level.', { name: me.name }) };
  const parent = codes.find((c) => c.id === me.parent_id);
  const grand = parent ? parent.parent_id : null;
  const parentSiblings = childrenOf(codes, grand).filter((c) => c.id !== id);
  const at = parent ? parentSiblings.findIndex((c) => c.id === parent.id) + 1 : parentSiblings.length;
  return { ok: true, plan: { parent_id: grand, position: at, where: grand === null ? t('to the top level') : t('under {name}', { name: codes.find((c) => c.id === grand)?.name ?? t('its grandparent') }) } };
}

/** Where a code lands when dropped on, before, or after another code. */
export function planDrop(
  codes: readonly CodeRow[],
  movedId: number,
  targetId: number,
  position: 'on' | 'before' | 'after',
): MoveResult {
  const moved = codes.find((c) => c.id === movedId);
  const target = codes.find((c) => c.id === targetId);
  if (!moved || !target) return { ok: false, reason: t('That code is not in the codebook any more.') };
  if (movedId === targetId) return { ok: false, reason: t('A code cannot be moved onto itself.') };
  if (descendantIds(codes, movedId).has(targetId)) return { ok: false, reason: t('A code cannot be moved under one of its own sub-codes.') };
  if (position === 'on') return { ok: true, plan: { parent_id: targetId, where: t('under {name}', { name: target.name }) } };
  const siblings = childrenOf(codes, target.parent_id).filter((c) => c.id !== movedId);
  const at = siblings.findIndex((c) => c.id === targetId);
  const place = position === 'before' ? at : at + 1;
  return { ok: true, plan: { parent_id: target.parent_id, position: place, where: `${position} ${target.name}` } };
}
