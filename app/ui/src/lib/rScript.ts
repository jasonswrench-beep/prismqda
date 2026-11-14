// The recorded script: every page of the Statistics notebook as R code, with the data it read as a CSV file beside it. The code is in
// rcode.code.ts; the words of the script (what its comments say, in the language of the window) are here.

import { dateTime, t } from '../i18n';
import type { NotebookEntry } from '../state/statistics';
import type { StatChart, StatResult, StatTable, StatVariable, StatVariableKind } from '../types';
import type { RChart } from './rbase.code';
import { announce, helpersFor, pageCode, readData, type RPage, type RVar } from './rcode.code';
import { chartTitle, subjectName } from './charts';
import { analysisRows } from './statBoot';
import { resultTitle, variableLabel } from './statistics';

/** The line that opens the file the charts are drawn into: named after the data file, with its ending changed. */
const CHARTS_FILE = 'pdf(paste0(sub("-data$", "", tools::file_path_sans_ext("{{data-file}}")), "-charts.pdf"), width = 7, height = 5)';

/** The name of a variable's column in the data file, and what the script reads it by. */
export function columnName(v: { name: string; kind: StatVariableKind }): string {
  return variableLabel(v);
}

const strings = (x: unknown): string[] | undefined => (Array.isArray(x) && x.every((s) => typeof s === 'string') ? (x as string[]) : undefined);
const records = (x: unknown): { value: string }[] | undefined => (Array.isArray(x) ? (x as { value: string }[]).filter((c) => c && typeof c.value === 'string') : undefined);

/** The names a chart puts on its axes: what is along the bottom, and what is up the side. */
function chartNames(c: StatChart, r: StatResult): string[] {
  const variable = (index: number) => subjectName({ type: 'variable', index }, r);
  switch (c.kind) {
    case 'histogram':
    case 'bars':
      return [subjectName(c.subject, r), t('Cases')];
    case 'box':
    case 'means':
      return [variable(c.by), subjectName(c.subject, r)];
    case 'stacked':
      return [variable(c.row), variable(c.col)];
    case 'qq':
      return [t('Expected if normal'), t('Value')];
    default:
      return [subjectName(c.x, r), subjectName(c.y, r)];
  }
}

/** What the code of one page needs to know: the request, and what the engine settled when it ran (the groups, the order of the values). */
export function pageSpec(entry: NotebookEntry, variables: ReadonlyMap<string, StatVariable>): RPage {
  const r: StatResult = entry.result;
  const details = r.details as unknown as Record<string, unknown>;
  const vars: RVar[] = r.variables.map((v) => {
    const known = variables.get(v.key);
    return {
      key: v.key,
      label: columnName(v),
      numeric: v.measure === 'scale' || (v.measure === 'ordinal' && (known?.numeric ?? false)),
      categories: (known?.categories ?? []).map((c) => c.value),
    };
  });
  const boot = entry.request.options?.bootstrap;
  const whole = r.details.bootstrap_analysis;
  const fits = Array.isArray(details.fits) ? (details.fits as { kind: string }[]) : [];
  const clusterKey = boot?.cluster;
  const clusterVar = clusterKey ? variables.get(clusterKey) : undefined;
  return {
    procedure: entry.request.procedure,
    vars,
    options: entry.request.options ?? {},
    groups: r.groups.map((g) => g.value),
    kind: typeof details.kind === 'string' ? details.kind : undefined,
    rowValues: strings(details.row_values),
    colValues: strings(details.col_values),
    values: strings(details.values),
    levelsA: strings(details.levels_a),
    levelsB: strings(details.levels_b),
    counts: records(details.counts)?.map((c) => c.value) ?? records(details.categories)?.map((c) => c.value),
    mean: typeof details.mean === 'number' ? details.mean : undefined,
    sd: typeof details.sd === 'number' ? details.sd : undefined,
    estimated: typeof details.estimated === 'boolean' ? details.estimated : undefined,
    event: typeof (details.event as { value?: unknown } | undefined)?.value === 'string' ? (details.event as { value: string }).value : undefined,
    profile: typeof details.profile === 'boolean' ? details.profile : undefined,
    curveKinds: Array.isArray(details.fits) ? (details.fits as { kind: string; error?: string }[]).filter((f) => f.error === undefined).map((f) => f.kind) : undefined,
    charts: r.charts.map((c): RChart => ({ chart: c, title: chartTitle(c, r), names: chartNames(c, r) })),
    // The numbers a bootstrap of the whole analysis gives, as the page words them.
    bootRows:
      boot && (boot.analysis || boot.cluster) && whole
        ? analysisRows(r)
            .filter((row) => whole.items.some((i) => i.path === row.path))
            .map((row) => ({ path: row.path, label: row.label, p: row.p === true, key: /^fits\/(\d+)\//.test(row.path) ? fits[Number(/^fits\/(\d+)\//.exec(row.path)?.[1])]?.kind : undefined }))
        : undefined,
    clusterLabel: clusterVar ? columnName(clusterVar) : undefined,
  };
}

/** The code of one page, under a heading comment, as it is written in a script. */
export function pageScript(entry: NotebookEntry, number: number, variables: ReadonlyMap<string, StatVariable>): string {
  const title = `${t('Page {n}', { n: String(number) })}: ${resultTitle(entry.result)}`;
  const lines = [`# ---- ${title.replace(/\s+/g, ' ')} ----`, announce(title)];
  const boot = entry.request.options?.bootstrap;
  if (boot) {
    lines.push(`# ${t('The page also has bootstrap intervals ({resamples} resamples, seed {seed}). PrismQDA draws its resamples with a generator that is written out below in R (SplitMix64), so these are the same resamples and the same intervals.', { resamples: String(boot.resamples ?? 2000), seed: String(boot.seed ?? 1) })}`);
  }
  lines.push(pageCode(pageSpec(entry, variables)));
  return lines.join('\n');
}

/** A value for a CSV file: quoted when it holds a comma, a quote, a line break, or spaces at an end. */
function field(text: string | null): string {
  if (text === null) return '';
  return /[",\r\n]|^\s|\s$/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

/** The data as CSV text: a row for each case, in the order of the cases, a missing value as an empty field. */
export function dataCsv(table: StatTable): { csv: string; classes: Record<string, 'numeric' | 'character'> } {
  const names = table.columns.map((c) => columnName(c));
  const rows = [names.map(field).join(',')];
  for (let i = 0; i < table.cases.length; i++) rows.push(table.columns.map((c) => field(c.values[i])).join(','));
  const classes: Record<string, 'numeric' | 'character'> = {};
  table.columns.forEach((c, i) => {
    classes[names[i]] = c.numeric ? 'numeric' : 'character';
  });
  return { csv: rows.join('\n') + '\n', classes };
}

/** The variables a notebook needs in the data file: every variable of cases (a rule can name any of them), and the code variables its pages use. */
export function tableKeys(entries: NotebookEntry[], variables: readonly StatVariable[]): string[] {
  const known = new Set(variables.map((v) => v.key));
  const keys = variables.filter((v) => v.kind === 'attribute').map((v) => v.key);
  for (const e of entries) for (const k of e.request.variables) if (k.startsWith('code:') && known.has(k) && !keys.includes(k)) keys.push(k);
  return keys;
}

/**
 * The whole script: a note on what it is, the data read in, the helper functions the pages call, and each page in the order the pages
 * were run. `{{data-file}}` stands where the name of the data file goes.
 */
export function scriptText(entries: NotebookEntry[], variables: readonly StatVariable[], classes: Record<string, 'numeric' | 'character'>, title: string): string {
  const byKey = new Map(variables.map((v) => [v.key, v]));
  const pages = entries.map((e, i) => pageScript(e, i + 1, byKey));
  const body = pages.join('\n\n');
  const drawn = entries.some((e) => e.result.charts.length > 0);
  const head = [
    `# ${title}`,
    `# ${t('Written by PrismQDA on {date}. Run it with R, with the folder that holds this file and its data file as the working directory.', { date: dateTime(new Date(), { dateStyle: 'long' }) })}`,
    `# ${t('Each page of the notebook is a section, in the order the pages were run, and prints the numbers the page shows.')}`,
    `# ${t('The data file has a row for each case and a column for each variable of cases, and for each code the pages used. A variable made in the data editor is in it with the values it had when this was written.')}`,
    '',
    readData(classes),
    ...(drawn ? [`# ${t('The charts of the pages are drawn into a PDF file that is saved beside the data file.')}`, CHARTS_FILE] : []),
    '',
    helpersFor(body),
    '',
  ];
  return head.join('\n') + '\n' + body + '\n' + (drawn ? 'invisible(dev.off())\n' : '');
}
