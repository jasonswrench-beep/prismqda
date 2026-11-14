// The one door between the interface and the engine.
//
//   const info = await call('project.info');
//
// In a browser it posts {cmd, args} to /rpc (the development bridge or a test server). Inside the
// Tauri window it uses invoke('call', {cmd, args}). A failure always throws an ApiError, so screens
// can show `message` as it is and branch on `code`.

import type { ApiErrorShape, CommandName, Commands } from './types';
import { t } from './i18n';
import { engineText } from './i18n/engine';

export class ApiError extends Error implements ApiErrorShape {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
  }
}

/** Makes any thrown value into an ApiError. */
export function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) return e;
  if (e && typeof e === 'object') {
    const o = e as { code?: unknown; message?: unknown };
    if (typeof o.code === 'string' && typeof o.message === 'string') return new ApiError(o.code, engineText(o.message));
    if (typeof o.message === 'string') return new ApiError('internal', engineText(o.message));
  }
  if (typeof e === 'string' && e) return new ApiError('internal', engineText(e));
  return new ApiError('internal', t('Something went wrong that PrismQDA did not expect.'));
}

/** True when the page runs inside the Tauri window. */
export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

type Transport = (cmd: string, args: unknown) => Promise<unknown>;
let override: Transport | null = null;

/** For tests only: replaces both transports. Pass null to restore them. */
export function setTransportForTests(t: Transport | null): void {
  override = t;
}

async function viaTauri(cmd: string, args: unknown): Promise<unknown> {
  // Imported on demand so browser mode never loads the Tauri package.
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    return await invoke('call', { cmd, args });
  } catch (e) {
    throw toApiError(e);
  }
}

async function viaHttp(cmd: string, args: unknown): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch('/rpc', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ cmd, args }),
    });
  } catch {
    throw new ApiError('network', t('PrismQDA cannot reach its engine. Check that it is still running, then try again.'));
  }
  let body: { ok?: boolean; result?: unknown; error?: ApiErrorShape } | null = null;
  try {
    body = await response.json();
  } catch {
    body = null;
  }
  if (!body) {
    throw new ApiError('network', t('The engine gave an answer PrismQDA could not read (status {status}).', { status: response.status }));
  }
  if (body.ok) return body.result;
  throw toApiError(body.error ?? { code: 'internal', message: t('The engine reported a problem without saying what it was.') });
}

/** Where a player gets the recording linked to a document: the desktop window's own address for it, or the development bridge's. */
export function mediaUrl(sourceId: number): string {
  return inTauri() ? `http://media.localhost/${sourceId}` : `/media/${sourceId}`;
}

/** Where a window gets a picture: the desktop window's own address for it, or the development bridge's. The stamp is the picture's size, so another picture under the same number is not mistaken for the first. */
export function imageUrl(sourceId: number, stamp: number | string = ''): string {
  const base = inTauri() ? `http://media.localhost/image/${sourceId}` : `/image/${sourceId}`;
  return stamp === '' ? base : `${base}?v=${stamp}`;
}

/**
 * Where a window gets one page of a PDF drawn: the desktop window's own address for it, or the development bridge's. The width is how
 * many pixels wide the page is drawn; the stamp is the PDF's size, so another PDF under the same number is not mistaken for the first.
 */
export function pdfPageUrl(sourceId: number, page: number, widthPx: number, stamp: number | string = ''): string {
  const base = inTauri() ? `http://media.localhost/pdf/${sourceId}/${page}/${widthPx}` : `/pdf/${sourceId}/${page}/${widthPx}`;
  return stamp === '' ? base : `${base}?v=${stamp}`;
}

/** The widths a page is drawn at. A page is asked for at the first of these that is at least as wide as it is shown, so zooming a little does not draw it again. */
export const PAGE_WIDTHS = [480, 720, 1000, 1400, 2000, 3000] as const;

/** The width to ask the engine to draw a page at so it looks sharp at `shownPx` screen pixels (and `ratio` device pixels to one screen pixel). */
export function pageWidthFor(shownPx: number, ratio = 1): number {
  const wanted = Math.max(1, shownPx) * Math.max(1, ratio);
  return PAGE_WIDTHS.find((w) => w >= wanted) ?? PAGE_WIDTHS[PAGE_WIDTHS.length - 1];
}

/** Commands that only read: they never change the project file, so they never move the Saved indicator. */
const READS = /^(ping|text\.(read|write)|project\.info|undo\.next|search\.run|segments\.list|link\.(list|relations)|theme\.(overview|ai_run)|codebook\.versions|events\.list|import\.inspect|source\.get|source\.list|code\.list|coder\.list|case\.list|coding\.list|coding\.history|memo\.list|unit\.list|unit\.attributes|media\.(get|list)|image\.(get|list)|pdf\.get|region\.list|reliability\.(list|get)|sample\.draw|adjudication\.(queue|list)|merge\.preview|analytics\.(words|kwic)|stats\.(variables|run)|data\.get|variable\.(preview|rule)|export\..+)$/;

/** True when a command can change the project file. */
export function isWrite(cmd: string): boolean {
  return !READS.test(cmd);
}

export type WriteEvent = { phase: 'saving' } | { phase: 'saved' } | { phase: 'unchanged' } | { phase: 'failed'; error: ApiError };

/** A refusal for a reason the person can fix (an empty name, a read-only project) leaves the file as it was; only a failure of the file or the engine means the write was lost. */
const REAL_FAILURES = new Set(['database', 'io', 'json', 'network', 'internal']);
let writeObserver: ((e: WriteEvent) => void) | null = null;

/** The screens' Saved indicator listens here: it is told when a write to the project starts, ends, or fails. */
export function setWriteObserver(fn: ((e: WriteEvent) => void) | null): void {
  writeObserver = fn;
}

let argsTransform: ((cmd: string, args: unknown) => unknown) | null = null;

/** Every request passes through this on its way to the engine, so one rule (the working set) can narrow what the pages ask for. */
export function setArgsTransform(fn: ((cmd: string, args: unknown) => unknown) | null): void {
  argsTransform = fn;
}

export async function call<K extends CommandName>(cmd: K, args?: Commands[K]['args']): Promise<Commands[K]['result']> {
  const payload = argsTransform ? argsTransform(cmd, args ?? {}) : (args ?? {});
  const write = writeObserver !== null && isWrite(cmd);
  if (write) writeObserver?.({ phase: 'saving' });
  try {
    let result: unknown;
    if (override) result = await override(cmd, payload);
    else if (inTauri()) result = await viaTauri(cmd, payload);
    else result = await viaHttp(cmd, payload);
    if (write) writeObserver?.({ phase: 'saved' });
    return result as Commands[K]['result'];
  } catch (e) {
    if (write) {
      const error = toApiError(e);
      writeObserver?.(REAL_FAILURES.has(error.code) ? { phase: 'failed', error } : { phase: 'unchanged' });
    }
    throw e;
  }
}
