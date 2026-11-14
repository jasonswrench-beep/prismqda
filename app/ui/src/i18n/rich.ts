// A message with marks in it ("Press <kbd>Enter</kbd> to code {name}") becomes a row of text and elements. The
// program decides what a mark looks like (a function from the words inside it to an element), and the translation
// decides where the marked words go, so a sentence can be reordered without the styling being lost.

import { createElement, Fragment, type ReactNode } from 'react';

export type RichValue = ReactNode | ((inside: ReactNode) => ReactNode);
export type RichArgs = Record<string, RichValue>;

const TOKEN = /\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}|<(\/?)([a-z][a-z0-9]*)(\s*\/)?>/g;

interface Frame {
  tag: string | null;
  nodes: ReactNode[];
}

export function richNodes(template: string, args: RichArgs): ReactNode {
  const stack: Frame[] = [{ tag: null, nodes: [] }];
  let key = 0;
  const top = () => stack[stack.length - 1] as Frame;
  const push = (node: ReactNode) => {
    top().nodes.push(createElement(Fragment, { key: key++ }, node));
  };
  const wrap = (tag: string, inside: ReactNode[]): ReactNode => {
    const make = args[tag];
    const content = createElement(Fragment, null, ...inside);
    return typeof make === 'function' ? make(content) : inside.length ? content : (make ?? null);
  };
  let last = 0;
  for (const m of template.matchAll(TOKEN)) {
    const at = m.index ?? 0;
    if (at > last) push(template.slice(last, at));
    last = at + m[0].length;
    if (m[0] === '{{') push('{');
    else if (m[0] === '}}') push('}');
    else if (m[1]) {
      const v = args[m[1]];
      push(typeof v === 'function' ? v(null) : v === undefined ? m[0] : v);
    } else if (m[3]) {
      const tag = m[3];
      if (m[4]) {
        // <tag/>
        const v = args[tag];
        push(typeof v === 'function' ? v(null) : (v ?? null));
      } else if (!m[2]) {
        stack.push({ tag, nodes: [] });
      } else if (top().tag === tag) {
        const frame = stack.pop() as Frame;
        push(wrap(tag, frame.nodes));
      } else {
        push(m[0]); // a closing mark with nothing open: shown as written
      }
    }
  }
  if (last < template.length) push(template.slice(last));
  // Marks left open are closed at the end, so a damaged translation still shows all its words.
  while (stack.length > 1) {
    const frame = stack.pop() as Frame;
    push(wrap(frame.tag as string, frame.nodes));
  }
  return createElement(Fragment, null, ...(stack[0] as Frame).nodes);
}
