// Messages for screen readers (and for everyone, in the status line at the bottom of the window).
// A polite message waits for the reader to finish; an assertive one, used for errors, interrupts.
// The last messages are also kept, so the notification bell can list what was said.

import { createStore, useStore } from './store';

interface AnnounceState {
  polite: string;
  assertive: string;
  /** Changes with every message so the same words can be announced twice in a row. */
  seq: number;
}

export const announceStore = createStore<AnnounceState>({ polite: '', assertive: '', seq: 0 });

export interface Notice {
  id: number;
  text: string;
  mode: 'polite' | 'assertive';
  at: number;
}

interface NoticeState {
  /** Newest first. */
  items: Notice[];
  /** How many problems (assertive messages) have come since the list was last opened. */
  unread: number;
}

const HISTORY_MAX = 30;
export const noticeStore = createStore<NoticeState>({ items: [], unread: 0 });
let noticeId = 0;

/** Marks everything in the list as seen; the bell loses its dot. */
export function markNoticesSeen(): void {
  noticeStore.set({ unread: 0 });
}

export function useNotices(): NoticeState {
  const items = useStore(noticeStore, (s) => s.items);
  const unread = useStore(noticeStore, (s) => s.unread);
  return { items, unread };
}

const timers: Record<'polite' | 'assertive', ReturnType<typeof setTimeout> | undefined> = { polite: undefined, assertive: undefined };
let fadeTimer: ReturnType<typeof setTimeout> | undefined;

/** How long an error stays in the status line before it is cleared. */
const ERROR_SHOWN_MS = 12_000;

let lastActionAt = 0;

/** True if something other than a "where is the caret" note was announced in the last `ms` milliseconds. */
export function announcedRecently(ms: number): boolean {
  return Date.now() - lastActionAt < ms;
}

/**
 * Says something. The text appears in the status line too. Pass `background: true` for notes that
 * are only helpful when nothing more important was just said (such as which codes the caret is in).
 */
export function announce(message: string, mode: 'polite' | 'assertive' = 'polite', opts?: { background?: boolean }): void {
  if (!opts?.background) lastActionAt = Date.now();
  if (!opts?.background && message) {
    noticeStore.set((s) => ({
      items: [{ id: ++noticeId, text: message, mode, at: Date.now() }, ...s.items].slice(0, HISTORY_MAX),
      unread: mode === 'assertive' ? s.unread + 1 : s.unread,
    }));
  }
  // Clearing first makes a screen reader treat the next text as new even if it is the same words.
  announceStore.set((s) => ({ [mode]: '', seq: s.seq + 1 }) as Partial<AnnounceState>);
  const pending = timers[mode];
  if (pending) clearTimeout(pending);
  timers[mode] = setTimeout(() => {
    announceStore.set((s) => ({ [mode]: message, seq: s.seq + 1 }) as Partial<AnnounceState>);
    if (mode === 'assertive') {
      if (fadeTimer) clearTimeout(fadeTimer);
      fadeTimer = setTimeout(() => announceStore.set({ assertive: '' }), ERROR_SHOWN_MS);
    }
  }, 40);
}

export function useAnnouncements(): AnnounceState {
  const polite = useStore(announceStore, (s) => s.polite);
  const assertive = useStore(announceStore, (s) => s.assertive);
  const seq = useStore(announceStore, (s) => s.seq);
  return { polite, assertive, seq };
}
