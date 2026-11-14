// The engine says a few things in English that the window shows as they come (why a request was refused, a note under a table, a
// warning after an import). engine-messages.json holds those sentences as templates with named blanks, collected from the Rust
// source by scripts/i18n/engine-extract.mjs, and goes into the catalog like every other message. engineText looks a sentence up
// by its shape and, for a language other than English, gives its translation with the blanks filled.

import data from './engine-messages.json';
import { getLocaleState, t } from './index';

interface Compiled {
  template: string;
  names: string[];
  re: RegExp;
  /** How many characters of the template are fixed words: the more, the more surely a sentence is this one. */
  weight: number;
}

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

let exact: Map<string, string> | null = null;
let shaped: Compiled[] | null = null;

function build(): void {
  exact = new Map();
  const list: Compiled[] = [];
  for (const { template } of data.messages as { template: string }[]) {
    const names: string[] = [];
    let re = '';
    let weight = 0;
    let last = 0;
    const piece = (s: string) => {
      weight += s.length;
      return escapeRe(s);
    };
    for (const m of template.matchAll(/\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}/g)) {
      re += piece(template.slice(last, m.index));
      if (m[0] === '{{') re += '\\{';
      else if (m[0] === '}}') re += '\\}';
      else {
        names.push(m[1] as string);
        re += '([\\s\\S]*?)';
      }
      last = (m.index ?? 0) + m[0].length;
    }
    re += piece(template.slice(last));
    if (names.length === 0) exact.set(template.replace(/\{\{/g, '{').replace(/\}\}/g, '}'), template);
    else list.push({ template, names, re: new RegExp(`^${re}$`), weight });
  }
  shaped = list.sort((a, b) => b.weight - a.weight);
}

/** A sentence from the engine, in the language of the window when the catalog has it; else as it came. */
export function engineText(text: string): string {
  if (!text) return text;
  if (getLocaleState().code === 'en') return text;
  if (!exact || !shaped) build();
  const whole = exact?.get(text);
  if (whole !== undefined) return t(whole);
  for (const c of shaped ?? []) {
    const m = c.re.exec(text);
    if (!m) continue;
    const args: Record<string, string> = {};
    c.names.forEach((n, i) => {
      args[n] = m[i + 1] ?? '';
    });
    return t(c.template, args);
  }
  return text;
}
