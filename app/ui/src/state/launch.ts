// A project file double clicked in Windows Explorer.
//
// The installer makes `.pqda` open with PrismQDA, and Windows then starts the program with the file's path on its command line.
// The program (src-tauri/src/main.rs) keeps that path, and when PrismQDA is already open it is the running window that is sent the
// path by the second start. Either way the window asks for the path with the `launch_path` command and opens the project the way
// the Recent Projects list does (an encrypted one asks for its passphrase first).

import { inTauri, toApiError } from '../bridge';
import { t } from '../i18n';
import { announce } from './announce';
import { openProject } from './actions';
import { appStore } from './app';

/** True when two paths name the same file as Windows sees it: the same letters whatever their case, and either kind of slash. */
export function samePath(a: string, b: string): boolean {
  const plain = (p: string) => p.replace(/^\\\\\?\\/, '').replace(/\//g, '\\').toLowerCase();
  return plain(a) === plain(b);
}

/** The project file the program was started with or was sent, handed over once; null when there is none (or outside the Windows program). */
export async function takeLaunchPath(): Promise<string | null> {
  if (!inTauri()) return null;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const path = await invoke<string | null>('launch_path');
    return typeof path === 'string' && path !== '' ? path : null;
  } catch {
    return null;
  }
}

/**
 * Opens a project the person double clicked. One that is already open is left as it is (the person keeps their place) and the
 * window says so. A file that cannot be opened is said in the status bar, in words, and the window stays where it was.
 */
export async function openLaunchedProject(path: string): Promise<void> {
  const open = appStore.get().project;
  if (open && samePath(open.path, path)) {
    announce(t('{name} is already open.', { name: open.name }));
    return;
  }
  try {
    await openProject(path);
  } catch (e) {
    const name = path.split(/[\\/]/).pop() ?? path;
    announce(t('{name} could not be opened: {message}', { name, message: toApiError(e).message }), 'assertive');
  }
}

/**
 * Opens the project the program was started with, if there is one, and then opens each one a later start sends while the window
 * is open. Call it once the window has started. Returns the function that stops listening. Does nothing outside the Windows program.
 */
export async function watchLaunchPaths(): Promise<() => void> {
  if (!inTauri()) return () => {};
  // Listening comes first, so a file sent while the first one is opening is not missed (the path waits in the program until asked for).
  let stop: () => void = () => {};
  try {
    const { listen } = await import('@tauri-apps/api/event');
    stop = await listen('launch-path', () => {
      void takeLaunchPath().then((path) => (path ? openLaunchedProject(path) : undefined));
    });
  } catch {
    // Without the listener, only the file the program was started with is opened.
  }
  const first = await takeLaunchPath();
  if (first) await openLaunchedProject(first);
  return stop;
}
