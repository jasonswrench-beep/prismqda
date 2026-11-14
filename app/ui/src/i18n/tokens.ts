// What a message is made of besides its words: blanks ({name}), marks (<b>...</b>), and the braces that stand for themselves.

const TOKEN = /\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}|<(\/?)([a-z][a-z0-9]*)(\s*\/)?>/g;

export interface MessageParts {
  /** The names of the blanks, in order of first use. */
  placeholders: string[];
  /** The names of the marks, in order of first use. */
  tags: string[];
  /** True when a brace stands alone, which a translation must not do (a plain { or } is written {{ or }}). */
  stray: boolean;
}

export function infoOf(template: string): MessageParts {
  const placeholders: string[] = [];
  const tags: string[] = [];
  for (const m of template.matchAll(TOKEN)) {
    if (m[1] && !placeholders.includes(m[1])) placeholders.push(m[1]);
    if (m[3] && !tags.includes(m[3])) tags.push(m[3]);
  }
  const rest = template.replace(/\{\{|\}\}/g, '').replace(/\{[A-Za-z_][A-Za-z0-9_]*\}/g, '');
  return { placeholders, tags, stray: /[{}]/.test(rest) };
}
