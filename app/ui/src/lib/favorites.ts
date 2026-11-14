// Favorite codes: up to nine, reached with the number keys 1 to 9.

export const MAX_FAVORITES = 9;

/** The list is positional: index 0 is key 1. A slot with no code holds null. */
export type Favorites = (number | null)[];

/** Cleans a list read from storage: numbers only, no repeats, at most nine slots. */
export function normalizeFavorites(raw: unknown, knownIds?: ReadonlySet<number>): Favorites {
  const out: Favorites = [];
  const seen = new Set<number>();
  if (Array.isArray(raw)) {
    for (const v of raw.slice(0, MAX_FAVORITES)) {
      if (typeof v === 'number' && Number.isInteger(v) && !seen.has(v) && (!knownIds || knownIds.has(v))) {
        seen.add(v);
        out.push(v);
      } else out.push(null);
    }
  }
  while (out.length > 0 && out[out.length - 1] === null) out.pop();
  return out;
}

/** Pins a code to the first free slot. Returns null if all nine are taken. */
export function pin(list: Favorites, id: number): Favorites | null {
  if (list.includes(id)) return list;
  const next = [...list];
  const free = next.indexOf(null);
  if (free >= 0) next[free] = id;
  else if (next.length < MAX_FAVORITES) next.push(id);
  else return null;
  return next;
}

/** Frees the slot; other codes keep their numbers so muscle memory is not broken. */
export function unpin(list: Favorites, id: number): Favorites {
  const next = list.map((v) => (v === id ? null : v));
  while (next.length > 0 && next[next.length - 1] === null) next.pop();
  return next;
}

/** The code for a number key (1 to 9), or null. */
export function codeForSlot(list: Favorites, slot: number): number | null {
  return list[slot - 1] ?? null;
}

export function slotOf(list: Favorites, id: number): number | null {
  const i = list.indexOf(id);
  return i >= 0 ? i + 1 : null;
}
