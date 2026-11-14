// The import queue: the files a person has added, what each one is, and where each stands. Plain data and plain rules, so
// "what happens when Import is pressed" can be checked without a screen.
import { t, tn } from '../i18n';

export type ItemStatus =
  /** Being read to see what is in it. */
  | 'checking'
  /** Read, and ready to go in. */
  | 'ready'
  /** A spreadsheet: waiting for the person to say what each column is. */
  | 'needs-choice'
  /** The same content is already in the project; waiting for the person to skip it or keep both. */
  | 'duplicate'
  | 'importing'
  | 'complete'
  /** Could not be read or could not be imported; it can be tried again. */
  | 'failed'
  /** Left out on purpose. */
  | 'skipped';

export const STATUS_LABELS: Record<ItemStatus, string> = {
  get checking() { return t('Checking'); },
  get ready() { return t('Ready'); },
  get ['needs-choice']() { return t('Needs a decision'); },
  get duplicate() { return t('Duplicate'); },
  get importing() { return t('Importing'); },
  get complete() { return t('Complete'); },
  get failed() { return t('Failed'); },
  get skipped() { return t('Skipped'); },
};

/** What the person decided about a file whose content is already in the project. */
export type DuplicateAction = 'skip' | 'again';

export interface QueueItem {
  id: number;
  path: string;
  status: ItemStatus;
  /** Why it failed, in words. */
  error?: string;
  duplicate: DuplicateAction;
}

/** The last part of a path, whichever kind of slash it uses. */
export function fileNameOf(path: string): string {
  const parts = path.replace(/[\\/]+$/, '').split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** The items that go in when Import is pressed: the ones that are ready, and duplicates the person chose to keep. */
export function toImport(items: readonly QueueItem[]): QueueItem[] {
  return items.filter((i) => i.status === 'ready' || (i.status === 'duplicate' && i.duplicate === 'again'));
}

/** Whether Import has anything to do, and why not when it does not. */
export function importBlocker(items: readonly QueueItem[]): string | null {
  if (items.length === 0) return t('Add a file first.');
  if (items.some((i) => i.status === 'checking' || i.status === 'importing')) return t('Wait for the files to be checked.');
  if (toImport(items).length > 0) return null;
  if (items.some((i) => i.status === 'needs-choice')) return t('Say what each column of the spreadsheet is for first.');
  if (items.some((i) => i.status === 'duplicate')) return t('Skip the duplicate or choose to import it again.');
  if (items.some((i) => i.status === 'failed')) return t('The files that failed can be tried again, or removed.');
  return t('Everything in the queue is done.');
}

export interface Tally {
  complete: number;
  failed: number;
  skipped: number;
  /** Still to be dealt with: not done, not failed, not skipped. */
  waiting: number;
}

export function tally(items: readonly QueueItem[]): Tally {
  const counts: Tally = { complete: 0, failed: 0, skipped: 0, waiting: 0 };
  for (const i of items) {
    if (i.status === 'complete') counts.complete++;
    else if (i.status === 'failed') counts.failed++;
    else if (i.status === 'skipped') counts.skipped++;
    else counts.waiting++;
  }
  return counts;
}

/** One sentence for the end of an import: "2 files imported, 1 failed, 1 skipped." */
export function outcomeLine(counts: Tally): string {
  const parts: string[] = [];
  if (counts.complete) parts.push(tn(counts.complete, { one: '{count} file imported', other: '{count} files imported' }));
  if (counts.failed) parts.push(t('{failed} failed', { failed: counts.failed }));
  if (counts.skipped) parts.push(t('{skipped} skipped', { skipped: counts.skipped }));
  if (counts.waiting) parts.push(t('{waiting} waiting', { waiting: counts.waiting }));
  return parts.length === 0 ? t('Nothing in the queue.') : `${parts.join(', ')}.`;
}

/** Items a retry should take up: the ones that failed. Completed items are never touched again. */
export function toRetry(items: readonly QueueItem[]): QueueItem[] {
  return items.filter((i) => i.status === 'failed');
}

/** A path already in the queue (the same file added twice) is not added a second time. */
export function alreadyQueued(items: readonly QueueItem[], path: string): boolean {
  const key = path.trim().toLowerCase();
  return items.some((i) => i.path.trim().toLowerCase() === key && i.status !== 'skipped');
}
