// Reading and writing the spreadsheets a translator works in. A spreadsheet saved as CSV can use commas, semicolons (what Excel
// writes in many countries), or tabs (Excel's "Unicode Text"); the delimiter is found from the first line. Excel may add a first
// line "sep=," to say which, and a byte-order mark, and both are accepted.

export interface ParsedCsv {
  rows: string[][];
  delimiter: string;
}

const DELIMITERS = [',', ';', '\t'];

/** The delimiter used by a header line: the one that appears most outside quotes (a comma if none). */
export function detectDelimiter(firstLine: string): string {
  const counts = new Map<string, number>(DELIMITERS.map((d) => [d, 0]));
  let quoted = false;
  for (const ch of firstLine) {
    if (ch === '"') quoted = !quoted;
    else if (!quoted && counts.has(ch)) counts.set(ch, (counts.get(ch) ?? 0) + 1);
  }
  let best = ',';
  let most = 0;
  for (const d of DELIMITERS) {
    const n = counts.get(d) ?? 0;
    if (n > most) {
      best = d;
      most = n;
    }
  }
  return best;
}

export function parseCsv(input: string): ParsedCsv {
  let text = input.charCodeAt(0) === 0xfeff ? input.slice(1) : input;
  let delimiter: string | null = null;
  const sep = text.match(/^sep=(.)\r?\n/i);
  if (sep) {
    delimiter = sep[1] as string;
    text = text.slice(sep[0].length);
  }
  if (delimiter === null) delimiter = detectDelimiter(text.split(/\r?\n/, 1)[0] ?? '');

  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let quoted = false;
  let sawAnything = false;
  const endCell = () => {
    row.push(cell);
    cell = '';
  };
  const endRow = () => {
    endCell();
    // a line with nothing on it is not a row
    if (!(row.length === 1 && row[0] === '')) rows.push(row);
    row = [];
    sawAnything = false;
  };
  for (let i = 0; i < text.length; i++) {
    const ch = text[i] as string;
    if (quoted) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          cell += '"';
          i++;
        } else quoted = false;
      } else cell += ch;
      continue;
    }
    if (ch === '"' && cell === '') {
      quoted = true;
      sawAnything = true;
    } else if (ch === delimiter) {
      endCell();
      sawAnything = true;
    } else if (ch === '\r') {
      if (text[i + 1] === '\n') i++;
      endRow();
    } else if (ch === '\n') {
      endRow();
    } else {
      cell += ch;
      sawAnything = true;
    }
  }
  if (sawAnything || cell !== '' || row.length > 0) endRow();
  return { rows, delimiter };
}

/** A cell a spreadsheet would read as a formula gets a tab in front of it, which it shows as plain text. */
export function guardCell(value: string): string {
  return /^[=+\-@\t\r]/.test(value) ? `\t${value}` : value;
}

/** Takes the guard off a cell read back. */
export function unguardCell(value: string): string {
  return value.startsWith('\t') ? value.slice(1) : value;
}

export function toCsv(rows: readonly (readonly string[])[], delimiter = ','): string {
  const quote = (v: string) => (/["\r\n]/.test(v) || v.includes(delimiter) || /^\s|\s$/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);
  return rows.map((r) => r.map((c) => quote(c)).join(delimiter)).join('\r\n') + '\r\n';
}
