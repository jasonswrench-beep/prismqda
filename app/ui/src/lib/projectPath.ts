// Where a new project goes: a name and a save location become one file path, and a name that Windows (or any system the
// file may travel to) would refuse is caught before anything is written.
import { t } from '../i18n';

const INVALID_CHARS = /[\\/:*?"<>|\u0000-\u001f]/;
const RESERVED = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i;
export const MAX_NAME = 100;

/** Why a project name cannot be used, in words for the person, or null when it can. */
export function validateProjectName(name: string): string | null {
  const trimmed = name.trim();
  if (trimmed === '') return t('Give the project a name.');
  if (Array.from(trimmed).length > MAX_NAME) return t('Use a name of {MAX_NAME} characters or fewer.', { MAX_NAME });
  const bad = trimmed.match(INVALID_CHARS);
  if (bad) return bad[0] === '"' ? t('The name cannot contain a quotation mark. Windows does not allow \\ / : * ? " < > | in a file name.') : t('The name cannot contain "{character}". Windows does not allow \\ / : * ? " < > | in a file name.', { character: bad[0] });
  if (/\.$/.test(trimmed)) return t('The name cannot end with a period.');
  if (RESERVED.test(trimmed)) return t('{name} is a name Windows keeps for itself. Choose another.', { name: trimmed });
  return null;
}

/** The file name a project name makes: unusable characters become dashes, and the .pqda ending is added. */
export function projectFileName(name: string): string {
  const base = name.trim().replace(new RegExp(INVALID_CHARS.source, 'g'), '-').replace(/[. ]+$/, '');
  return `${base === '' ? t('My study') : base}.pqda`;
}

/** Joins a folder and a file name with the kind of slash the folder uses. */
export function joinPath(folder: string, file: string): string {
  const f = folder.trim();
  if (f === '') return file;
  const slash = f.includes('\\') || /^[A-Za-z]:/.test(f) ? '\\' : '/';
  return f.endsWith('\\') || f.endsWith('/') ? `${f}${file}` : `${f}${slash}${file}`;
}

/**
 * The path a new project will be written to. A save location is normally a folder, and the file name comes from the project
 * name; a location that already ends in .pqda is taken as the whole path, so a full path typed or pasted still works.
 */
export function targetPath(location: string, name: string): string {
  const loc = location.trim().replace(/^"(.*)"$/, '$1');
  if (/\.pqda$/i.test(loc)) return loc;
  return joinPath(loc, projectFileName(name));
}

/** The folder and the file name of a path, for showing "Saved as Study.pqda in C:\Projects". */
export function splitPath(path: string): { folder: string; file: string } {
  const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return i < 0 ? { folder: '', file: path } : { folder: path.slice(0, i), file: path.slice(i + 1) };
}

/** A name that does not collide with the ones taken: "Study", then "Study 2", "Study 3". */
export function freeName(name: string, taken: readonly string[]): string {
  const lower = new Set(taken.map((t) => t.toLowerCase()));
  if (!lower.has(name.toLowerCase())) return name;
  for (let n = 2; n < 1000; n++) if (!lower.has(`${name} ${n}`.toLowerCase())) return `${name} ${n}`;
  return t('{name} copy', { name });
}
