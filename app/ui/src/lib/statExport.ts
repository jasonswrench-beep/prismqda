// What leaves the Statistics page: a chart as a picture, a table as text to paste, and the whole notebook as the document the engine
// writes (Word, Excel, or a web page). The words are the page's own, in the language of the window; the engine adds none.

import { currentLocale, dateTime, num, t } from '../i18n';
import type { NotebookEntry } from '../state/statistics';
import type { NotebookPayload, NotebookPayloadChart } from '../types';
import { chartDescription, chartTitle } from './charts';
import { assumptionLine, chosenCasesLine, resultReading, resultSentence, resultTables, resultTitle, usedLine, type ApaTable } from './statistics';

// ---------------------------------------------------------------- a chart as a drawing and as a picture

/** The style properties a chart's marks and words need once the page's style sheet is no longer there to give them. */
const DRAWN = [
  'fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-opacity', 'stroke-dasharray', 'stroke-linecap', 'stroke-linejoin', 'opacity',
  'font-size', 'font-weight', 'font-style', 'text-anchor', 'dominant-baseline', 'display', 'visibility',
];
/** The font the drawing asks for. The page's own is not in the file, so a font nearly everyone has comes first. */
const FONT = "'Segoe UI', Arial, Helvetica, sans-serif";

export interface Drawing {
  /** The SVG, as text, with every color and size written into it and a white background, in the light theme. */
  text: string;
  width: number;
  height: number;
}

/**
 * The SVG of a chart as a file of its own. A chart on the page takes its colors from the page's style sheet and the current theme;
 * a file has neither, so each element is given the values it has when the page is in its light theme, and the picture has a white
 * background whatever the theme the chart was looked at in.
 */
export function drawSvg(source: SVGSVGElement): Drawing {
  const root = document.documentElement;
  const was = root.getAttribute('data-theme');
  root.setAttribute('data-theme', 'light');
  try {
    const clone = source.cloneNode(true) as SVGSVGElement;
    const from: Element[] = [source, ...Array.from(source.querySelectorAll('*'))];
    const to: Element[] = [clone, ...Array.from(clone.querySelectorAll('*'))];
    from.forEach((el, i) => {
      const computed = getComputedStyle(el);
      const declarations = DRAWN.map((p) => `${p}:${computed.getPropertyValue(p)}`).filter((d) => !d.endsWith(':'));
      if (el.tagName.toLowerCase() === 'text' || el.tagName.toLowerCase() === 'tspan') declarations.push(`font-family:${FONT}`);
      const target = to[i];
      for (const a of ['class', 'tabindex', 'focusable', 'style']) target.removeAttribute(a);
      target.setAttribute('style', declarations.join(';'));
    });
    const box = source.viewBox.baseVal;
    const width = Math.round(box.width) || 640;
    const height = Math.round(box.height) || 360;
    clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    clone.setAttribute('width', String(width));
    clone.setAttribute('height', String(height));
    const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    for (const [k, v] of [['x', '0'], ['y', '0'], ['width', String(width)], ['height', String(height)], ['fill', '#ffffff']]) background.setAttribute(k, v);
    // The background is drawn first, and the name of the chart comes before it for anything that reads the file.
    clone.insertBefore(background, clone.firstChild);
    const label = source.getAttribute('aria-label');
    if (label) {
      const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
      title.textContent = label;
      clone.insertBefore(title, clone.firstChild);
    }
    return { text: new XMLSerializer().serializeToString(clone), width, height };
  } finally {
    if (was === null) root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', was);
  }
}

/** A drawing as the base 64 of a PNG, twice as many pixels as the drawing is wide so it stays sharp when it is enlarged. */
export async function drawingToPng(drawing: Drawing, scale = 2): Promise<string> {
  const image = new Image();
  const ready = new Promise<void>((resolve, reject) => {
    image.onload = () => resolve();
    image.onerror = () => reject(new Error(t('The chart could not be turned into a picture.')));
  });
  image.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(drawing.text);
  await ready;
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(drawing.width * scale);
  canvas.height = Math.round(drawing.height * scale);
  const context = canvas.getContext('2d');
  if (!context) throw new Error(t('The chart could not be turned into a picture.'));
  context.fillStyle = '#ffffff';
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const url = canvas.toDataURL('image/png');
  return url.slice(url.indexOf(',') + 1);
}

// ---------------------------------------------------------------- a table to paste

/** A table as tab-separated text, which a spreadsheet pastes into cells, and as HTML, which Word pastes as a table. */
export function tableForClipboard(table: ApaTable): { text: string; html: string } {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const plain = (cell: string) => cell.replace(/[\t\r\n]+/g, ' ');
  const text = [table.head, ...table.rows].map((row) => row.map(plain).join('\t')).join('\n');
  const head = `<tr>${table.head.map((c) => `<th>${esc(c)}</th>`).join('')}</tr>`;
  const body = table.rows.map((row) => `<tr>${row.map((c) => `<td>${esc(c)}</td>`).join('')}</tr>`).join('');
  const html = `<table><caption>${esc(table.title)}</caption><thead>${head}</thead><tbody>${body}</tbody></table>${table.note ? `<p>${esc(table.note)}</p>` : ''}`;
  return { text, html };
}

/** Puts a table on the clipboard as both kinds. Resolves false when the browser would not allow it. */
export async function copyTable(table: ApaTable): Promise<boolean> {
  const { text, html } = tableForClipboard(table);
  try {
    if (typeof ClipboardItem !== 'undefined' && navigator.clipboard?.write) {
      await navigator.clipboard.write([new ClipboardItem({ 'text/plain': new Blob([text], { type: 'text/plain' }), 'text/html': new Blob([html], { type: 'text/html' }) })]);
      return true;
    }
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------- the notebook

/** The words the file adds round the pages, in the language of the window. */
export function notebookLabels() {
  return {
    table: t('Table'),
    figure: t('Figure'),
    note: t('Note.'),
    contents: t('Contents'),
    checks: t('What the test depends on'),
    alternative: t('Text alternative'),
  };
}

/**
 * The notebook as the engine writes it. The charts are taken from the page as it is drawn (a chart that is not on the page is sent
 * as its words alone); a PNG is made for each only when `pictures` says the file will show it (Word and Excel), and the drawing is
 * sent only for the web page.
 */
export async function notebookPayload(entries: NotebookEntry[], options: { title: string; pictures: boolean; drawings: boolean; footer: string }): Promise<NotebookPayload> {
  const pages: NotebookPayload['pages'] = [];
  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i];
    const r = entry.result;
    const charts: NotebookPayloadChart[] = [];
    for (let c = 0; c < r.charts.length; c++) {
      const chart = r.charts[c];
      const figure = document.getElementById(`stats-chart-${entry.id}-${c}`);
      const svg = figure?.querySelector('svg.stats-svg') as SVGSVGElement | null;
      const out: NotebookPayloadChart = { title: chartTitle(chart, r), alt: chartDescription(chart, r), width: 640, height: 360 };
      if (svg) {
        const drawing = drawSvg(svg);
        out.width = drawing.width;
        out.height = drawing.height;
        if (options.drawings) out.svg = drawing.text;
        if (options.pictures) out.png = await drawingToPng(drawing);
      }
      charts.push(out);
    }
    const ranAt = Number.isNaN(new Date(entry.ranAt).getTime()) ? '' : dateTime(new Date(entry.ranAt), { dateStyle: 'medium', timeStyle: 'short' });
    const notes = [`${usedLine(r)} ${ranAt}`.trim()];
    if (entry.note.trim() !== '') notes.push(t('Note: {note}', { note: entry.note.trim() }));
    if (entry.request.options?.filter) notes.push(t('Only cases where: {rule}', { rule: entry.request.options.filter }));
    const chosen = chosenCasesLine(entry.request.options, true);
    if (chosen) notes.push(chosen);
    pages.push({
      title: `${t('Page {n}', { n: num(i + 1) })}: ${resultTitle(r)}`,
      sentence: resultSentence(r),
      reading: resultReading(r),
      notes,
      tables: resultTables(r).map((tb) => ({ title: tb.title, head: tb.head, rows: tb.rows, note: tb.note ?? '' })),
      checks: r.assumptions.map((a) => {
        const line = assumptionLine(a);
        return `${line.badge}: ${line.text}`;
      }),
      charts,
    });
  }
  return { title: options.title, language: currentLocale(), labels: notebookLabels(), footer: options.footer, pages };
}
