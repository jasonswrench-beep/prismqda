// Split view: which document goes beside the one being read, and what to do when the choice stops making sense.
// Plain functions, so they can be tested without a screen.

import type { SourceRow } from '../types';
import { folderParts } from './docTree';
import { compareText, t } from '../i18n';

/**
 * The document to show beside `currentId` when split view is turned on: the next document in its folder, else the one before it,
 * else any other document. `null` when there is no other document to show.
 */
export function defaultSplitTarget(sources: readonly SourceRow[], currentId: number): number | null {
  const current = sources.find((s) => s.id === currentId);
  if (!current) return sources[0]?.id ?? null;
  // The documents of its folder, in the order the Documents tree lists them.
  const key = folderParts(current.folder).join('/');
  const set = sources
    .filter((s) => folderParts(s.folder).join('/') === key)
    .sort((a, b) => compareText(a.name, b.name, { numeric: true, sensitivity: 'base' }));
  const at = set.findIndex((s) => s.id === currentId);
  const next = set[at + 1] ?? set[at - 1];
  if (next) return next.id;
  return sources.find((s) => s.id !== currentId)?.id ?? null;
}

/** The documents that can be shown beside `currentId`: every other one, in the order of the list of documents. */
export function splitChoices(sources: readonly SourceRow[], currentId: number | null): SourceRow[] {
  return sources.filter((s) => s.id !== currentId);
}

/**
 * What to say in the pane when it cannot show its document: it is the one being read, it is gone, or it is too large to be
 * drawn beside another. `null` when it can be shown.
 */
export function splitProblem(split: SourceRow | null, currentId: number | null, big: boolean): string | null {
  if (split === null) return t('That document is no longer in the project. Choose another to show here.');
  if (split.id === currentId) return t('This is the document being read on the left. Choose another to show here.');
  if (big) return t('{name} is a large document, which is too much to draw beside another one. Swap sides to read it on its own.', { name: split.name });
  return null;
}
