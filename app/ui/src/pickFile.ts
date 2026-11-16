// The single place where the interface asks the person for a file or folder path.
//
// In a browser there is no native file dialog we can use, so the person types the path and this
// module returns what they typed. Inside the Tauri window the same screens get Browse buttons that
// open Windows' own open and save dialogs, and a path left empty opens the dialog when the person
// presses the main button. Nothing else in the interface needs to know which of the two it is.

import { inTauri } from './bridge';
import { t } from './i18n';

export type PickPurpose = 'open-project' | 'new-project' | 'project-copy' | 'project-folder' | 'import-file' | 'import-folder' | 'export-file' | 'language-file' | 'key-file' | 'qdpx-file' | 'qdc-file' | 'class-folder';

/** Whether a Browse button should be shown next to path fields. */
export const nativeDialogsAvailable = inTauri();

/** What the importers can read, by file ending. Kept beside the engine's list in `Format::from_name`. */
const IMPORT_ENDINGS = [
  'txt', 'text', 'md', 'markdown', 'html', 'htm', 'docx', 'odt', 'rtf', 'pdf', 'csv', 'tsv', 'tab', 'xlsx', 'xlsm', 'sav', 'zsav', 'pstat', 'vtt', 'srt', 'json',
  'ris', 'bib', 'bibtex', 'eml', 'mbox', 'mbx', 'mht', 'mhtml', 'rss', 'atom', 'xml',
  'mp3', 'wav', 'm4a', 'aac', 'flac', 'ogg', 'oga', 'opus', 'wma', 'mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v',
  'png', 'jpg', 'jpeg', 'jpe', 'jfif', 'gif', 'webp', 'bmp',
];

/** The kinds of material on the Import Data page that can be imported, each with the file endings it reads. */
export type ImportKindId = 'documents' | 'reference' | 'web' | 'email' | 'media' | 'image';
const KIND_FILTERS: Record<ImportKindId, { name: string; extensions: string[] }> = {
  documents: { get name() { return t('Documents, spreadsheets, transcripts, and data (including a PrismStatistics project)'); }, extensions: ['txt', 'text', 'md', 'markdown', 'html', 'htm', 'docx', 'odt', 'rtf', 'pdf', 'csv', 'tsv', 'tab', 'xlsx', 'xlsm', 'sav', 'zsav', 'pstat', 'vtt', 'srt', 'json'] },
  reference: { get name() { return t('Reference library (RIS, BibTeX, CSL JSON, or a Reference Manager exchange file)'); }, extensions: ['ris', 'bib', 'bibtex', 'json'] },
  web: { get name() { return t('Saved web pages and news feeds'); }, extensions: ['mht', 'mhtml', 'html', 'htm', 'rss', 'atom', 'xml'] },
  email: { get name() { return t('E-mail messages and mailboxes'); }, extensions: ['eml', 'mbox', 'mbx'] },
  image: { get name() { return t('Pictures (PNG, JPEG, GIF, WebP, BMP)'); }, extensions: ['png', 'jpg', 'jpeg', 'jpe', 'jfif', 'gif', 'webp', 'bmp'] },
  media: { get name() { return t('Audio and video recordings (captions beside them are optional)'); }, extensions: ['mp3', 'wav', 'm4a', 'aac', 'flac', 'ogg', 'oga', 'opus', 'wma', 'mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v'] },
};
const PROJECT_FILTER = [{ get name() { return t('PrismQDA project'); }, extensions: ['pqda'] }];

/** Removes the quotes Windows adds when a path is copied with "Copy as path", and surrounding spaces. */
export function cleanTypedPath(typed: string): string {
  let t = typed.trim();
  if (t.length >= 2 && ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'")))) {
    t = t.slice(1, -1).trim();
  }
  return t;
}

/** A file name made from a project name: characters Windows does not allow become dashes. */
export function suggestedProjectFile(name: string): string {
  const base = name.trim().replace(/[\\/:*?"<>|]/g, '-');
  return `${base === '' ? t('My study') : base}.pqda`;
}

/**
 * Always shows the native dialog. Returns the chosen path, or null if the person cancelled or there is no
 * native dialog (in a browser). `suggestion` is the project name, used to propose a file name.
 */
export async function browsePath(purpose: PickPurpose, suggestion = '', ending = ''): Promise<string | null> {
  if (!inTauri()) return null;
  const dialog = await import('@tauri-apps/plugin-dialog');
  if (purpose === 'export-file') {
    const chosen = await dialog.save({
      title: t('Where to save the file'),
      defaultPath: suggestion === '' ? undefined : suggestion,
      filters: ending === '' ? undefined : [{ name: ending.toUpperCase() + ' file', extensions: [ending] }],
    });
    return chosen ?? null;
  }
  if (purpose === 'import-folder') {
    const chosen = await dialog.open({ title: t('Choose a folder to import'), multiple: false, directory: true });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'project-folder') {
    const chosen = await dialog.open({ title: t('Choose the folder to save the project in'), multiple: false, directory: true });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'class-folder') {
    const chosen = await dialog.open({ title: t('Choose the folder for the class packets'), multiple: false, directory: true });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'new-project') {
    const chosen = await dialog.save({ title: t('Where to save the new project'), defaultPath: suggestedProjectFile(suggestion), filters: PROJECT_FILTER });
    return chosen ?? null;
  }
  if (purpose === 'project-copy') {
    const chosen = await dialog.save({ title: t('Where to save the copy'), defaultPath: suggestedProjectFile(suggestion), filters: PROJECT_FILTER });
    return chosen ?? null;
  }
  if (purpose === 'qdpx-file') {
    const chosen = await dialog.open({
      title: t('Choose a QDPX project to bring in'),
      multiple: false,
      directory: false,
      filters: [{ name: t('QDPX project (REFI-QDA)'), extensions: ['qdpx'] }, { name: t('All files'), extensions: ['*'] }],
    });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'qdc-file') {
    const chosen = await dialog.open({
      title: t('Choose a codebook to bring in'),
      multiple: false,
      directory: false,
      filters: [{ name: t('Codebook (REFI-QDA)'), extensions: ['qdc'] }, { name: t('All files'), extensions: ['*'] }],
    });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'key-file') {
    const chosen = await dialog.open({
      title: t('Choose a key file'),
      multiple: false,
      directory: false,
      filters: [{ name: t('Key files (CSV)'), extensions: ['csv', 'txt'] }, { name: t('All files'), extensions: ['*'] }],
    });
    return typeof chosen === 'string' ? chosen : null;
  }
  if (purpose === 'language-file') {
    const chosen = await dialog.open({
      title: t('Choose a translation file'),
      multiple: false,
      directory: false,
      filters: [{ name: t('Translation files (CSV or JSON)'), extensions: ['csv', 'json', 'tsv', 'txt'] }, { name: t('All files'), extensions: ['*'] }],
    });
    return typeof chosen === 'string' ? chosen : null;
  }
  const chosen = await dialog.open({
    title: purpose === 'open-project' ? t('Open a project') : t('Choose a file to import'),
    multiple: false,
    directory: false,
    filters: purpose === 'open-project' ? PROJECT_FILTER : [{ name: t('Documents, spreadsheets, transcripts, and data'), extensions: IMPORT_ENDINGS }, { name: t('All files'), extensions: ['*'] }],
  });
  return typeof chosen === 'string' ? chosen : null;
}

/** Several files at once, from Windows' own dialog. Empty in a browser, where the person types a path instead. */
export async function browseManyFiles(kind: ImportKindId = 'documents'): Promise<string[]> {
  if (!inTauri()) return [];
  const dialog = await import('@tauri-apps/plugin-dialog');
  const chosen = await dialog.open({
    title: t('Choose files to import'),
    multiple: true,
    directory: false,
    filters: [KIND_FILTERS[kind], { name: t('Everything PrismQDA can import'), extensions: IMPORT_ENDINGS }, { name: t('All files'), extensions: ['*'] }],
  });
  return Array.isArray(chosen) ? chosen : typeof chosen === 'string' ? [chosen] : [];
}

/**
 * Listens for files dropped on the window. The window says where each file is (a web page cannot), so this only works
 * inside the desktop window. `onOver` is told when files are being dragged over it. Returns a function that stops listening.
 */
export async function listenForFileDrops(onDrop: (paths: string[]) => void, onOver: (over: boolean) => void): Promise<() => void> {
  if (!inTauri()) return () => undefined;
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  return getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === 'enter' || p.type === 'over') onOver(true);
    else if (p.type === 'leave') onOver(false);
    else if (p.type === 'drop') {
      onOver(false);
      onDrop(p.paths);
    }
  });
}

/**
 * Returns the path to use, or null if the person gave none (or cancelled the native dialog).
 * `typed` is whatever is in the text field now; it wins when it is not empty.
 */
export async function pickPath(purpose: PickPurpose, typed: string, suggestion = '', ending = ''): Promise<string | null> {
  const path = cleanTypedPath(typed);
  if (path !== '') return path;
  return browsePath(purpose, suggestion, ending);
}
