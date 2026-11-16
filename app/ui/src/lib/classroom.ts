// The small functions of a class exercise (decision 183) that need no engine: reading a pasted list of students, naming the folder the class's
// packets go in, and the words of the figures in the review. The numbers themselves are the engine's.

import { num, t } from '../i18n';

/** The students in a pasted list: one on each line, with spaces squeezed and empty lines dropped. A name repeated (any case) is kept once, in its first place. */
export function parseStudents(text: string): { names: string[]; repeated: string[] } {
  const seen = new Set<string>();
  const names: string[] = [];
  const repeated: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const name = line.replace(/\s+/g, ' ').trim();
    if (name === '') continue;
    if (seen.has(name.toLowerCase())) repeated.push(name);
    else {
      seen.add(name.toLowerCase());
      names.push(name);
    }
  }
  return { names, repeated };
}

/** The folder suggested for a class's packets: beside the project, named for it. */
export function suggestedClassFolder(projectPath: string): string {
  const slash = Math.max(projectPath.lastIndexOf('\\'), projectPath.lastIndexOf('/'));
  const folder = projectPath.slice(0, slash + 1);
  const stem = projectPath.slice(slash + 1).replace(/\.pqda$/i, '');
  return folder + t('{stem} class packets', { stem: stem || t('Class') });
}

/** A share (0 to 1) as a whole percent, or a dash when there was nothing to divide. */
export function percentText(share: number | null | undefined): string {
  return share === null || share === undefined || !Number.isFinite(share) ? '–' : t('{percent}%', { percent: num(Math.round(share * 100)) });
}

/** A kappa as a number with two places, or "not defined". */
export function kappaText(kappa: number | null | undefined): string {
  return kappa === null || kappa === undefined || !Number.isFinite(kappa) ? t('not defined') : num(kappa, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/**
 * How dark a cell of the class heat map is, from 0 (every passage with the code was found) to 4 (fewer than a quarter were). The number in the
 * cell says the same thing; the shading is a help for seeing where the class struggled.
 */
export function heatLevel(share: number | null | undefined): 0 | 1 | 2 | 3 | 4 {
  if (share === null || share === undefined || !Number.isFinite(share) || share >= 1) return 0;
  if (share >= 0.75) return 1;
  if (share >= 0.5) return 2;
  if (share >= 0.25) return 3;
  return 4;
}

/** The file name suggested for a table of the class: the heat map or the side-by-side view. */
export function suggestedClassTable(projectName: string, what: string): string {
  const base = projectName.trim().replace(/[\\/:*?"<>|]/g, '-') || t('Class');
  return `${base} ${what}.csv`;
}

/** The file name suggested for the table of students. */
export function suggestedReviewFile(projectName: string): string {
  const base = projectName.trim().replace(/[\\/:*?"<>|]/g, '-') || t('Class');
  return `${base} ${t('class review')}.csv`;
}
