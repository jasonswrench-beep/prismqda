// Comparing two states of the codebook: a frozen version against another, or against the codebook today. A code keeps its
// identity when its name or parent changes, so codes are matched by id and a rename is a change, not a removal and an addition.

import { t, tn } from '../i18n';

/** The parts of a code that a version remembers. */
export interface SnapCode {
  id: number;
  name: string;
  parent_id: number | null;
  kind: string;
  color: string;
  definition: string;
  inclusion_criteria: string;
  exclusion_criteria: string;
  status: string;
  /** Properties came with format 3; a version saved before then has none, which reads the same as the defaults. */
  exclusive_group?: string;
  rated?: boolean;
  rating_label?: string;
  rating_low?: string;
  rating_high?: string;
}

export type Field =
  | 'name'
  | 'parent'
  | 'kind'
  | 'definition'
  | 'inclusion_criteria'
  | 'exclusion_criteria'
  | 'status'
  | 'color'
  | 'exclusive_group'
  | 'rated'
  | 'rating_label'
  | 'rating_low'
  | 'rating_high';

export const FIELD_LABELS: Record<Field, string> = {
  get name() { return t('Title'); },
  get parent() { return t('Place in the hierarchy'); },
  get kind() { return t('Kind'); },
  get definition() { return t('Description'); },
  get inclusion_criteria() { return t('Inclusion Criteria'); },
  get exclusion_criteria() { return t('Exclusion Criteria'); },
  get status() { return t('State'); },
  get color() { return t('Color'); },
  get exclusive_group() { return t('Mutually Exclusive Group'); },
  get rated() { return t('Weighted / Rated'); },
  get rating_label() { return t('Rating name'); },
  get rating_low() { return t('A 1 means'); },
  get rating_high() { return t('A 5 means'); },
};

export interface FieldChange {
  field: Field;
  label: string;
  from: string;
  to: string;
}

export interface CodeChange {
  id: number;
  /** The name now (in the later of the two). */
  name: string;
  changes: FieldChange[];
}

export interface CodebookDiff {
  added: SnapCode[];
  removed: SnapCode[];
  changed: CodeChange[];
}

/** "Access to Care / Transportation": a code's place in the hierarchy, as words (the code's own name last). */
function placeOf(codes: ReadonlyMap<number, SnapCode>, code: SnapCode): string {
  const names: string[] = [];
  let cur: SnapCode | undefined = code;
  for (let guard = 0; cur && guard < 20; guard++) {
    names.unshift(cur.name);
    cur = cur.parent_id === null ? undefined : codes.get(cur.parent_id);
  }
  return names.join(' / ');
}

const KIND_WORDS: Record<string, string> = { get theme() { return t('Theme'); }, get code() { return t('Code'); } };
const STATE_WORDS: Record<string, string> = { get active() { return t('Active'); }, get retired() { return t('Retired'); }, get merged() { return t('Merged into another code'); } };

const FIELDS: Field[] = ['name', 'parent', 'kind', 'definition', 'inclusion_criteria', 'exclusion_criteria', 'status', 'color', 'exclusive_group', 'rated', 'rating_label', 'rating_low', 'rating_high'];

/** What changed between `before` and `after`. Added and removed codes are listed whole; a changed code lists its changed fields. */
export function diffCodebooks(before: readonly SnapCode[], after: readonly SnapCode[]): CodebookDiff {
  const b = new Map(before.map((c) => [c.id, c]));
  const a = new Map(after.map((c) => [c.id, c]));
  const added = after.filter((c) => !b.has(c.id));
  const removed = before.filter((c) => !a.has(c.id));
  const changed: CodeChange[] = [];
  for (const now of after) {
    const was = b.get(now.id);
    if (!was) continue;
    const changes: FieldChange[] = [];
    for (const field of FIELDS) {
      let from: string;
      let to: string;
      if (field === 'parent') {
        from = was.parent_id === null ? t('(top level)') : t('under {place}', { place: placeOf(b, b.get(was.parent_id) ?? was) });
        to = now.parent_id === null ? t('(top level)') : t('under {place}', { place: placeOf(a, a.get(now.parent_id) ?? now) });
        if (was.parent_id === null && now.parent_id === null) continue;
      } else if (field === 'kind') {
        from = KIND_WORDS[was.kind] ?? was.kind;
        to = KIND_WORDS[now.kind] ?? now.kind;
      } else if (field === 'status') {
        from = STATE_WORDS[was.status] ?? was.status;
        to = STATE_WORDS[now.status] ?? now.status;
      } else if (field === 'rated') {
        from = was.rated ? t('Yes') : t('No');
        to = now.rated ? t('Yes') : t('No');
      } else if (field === 'exclusive_group') {
        from = was.exclusive_group || t('None');
        to = now.exclusive_group || t('None');
      } else {
        from = String(was[field] ?? '');
        to = String(now[field] ?? '');
      }
      if (from !== to) changes.push({ field, label: FIELD_LABELS[field], from, to });
    }
    if (changes.length > 0) changed.push({ id: now.id, name: now.name, changes });
  }
  return { added, removed, changed };
}

/** True when the two codebooks are the same in everything compared. */
export function isSame(d: CodebookDiff): boolean {
  return d.added.length === 0 && d.removed.length === 0 && d.changed.length === 0;
}

/** One line in words for the summary: "1 code added, 2 changed". */
export function summarizeDiff(d: CodebookDiff): string {
  if (isSame(d)) return t('No differences.');
  const parts: string[] = [];
  if (d.added.length) parts.push(tn(d.added.length, { one: '{count} code added', other: '{count} codes added' }));
  if (d.removed.length) parts.push(tn(d.removed.length, { one: '{count} code removed', other: '{count} codes removed' }));
  if (d.changed.length) parts.push(tn(d.changed.length, { one: '{count} code changed', other: '{count} codes changed' }));
  return `${parts.join(', ')}.`;
}
