// Speaker labels in a transcript: a line that starts with a name and a colon ("Interviewer: ", "Alex: ") is drawn with the
// name in a column of its own and the words hanging beside it, as in a transcript in print. The label stays part of the
// text (nothing is cut out of the source); it is only drawn differently, so every offset stays what the engine has.

import type { OffsetIndex } from '../offsets';

export interface LabelRange {
  /** Scalar offsets: the first character of the name, and the first character after the colon and the spaces. */
  start: number;
  end: number;
}

const LABEL = /(?:^|\n)(\p{Lu}[^\n:]{0,28}:)([ \t]+)/gu;
const NOT_NAMES = /^(https?|ftp|mailto|note|notes|nb|ps|re|fwd|cc|bcc|to|from|subject|date|time|location|page)$/i;

/** The labels at the start of lines, in text order. A label is at most four words, each starting with a capital letter or a digit. */
export function findSpeakerLabels(text: string, index: OffsetIndex): LabelRange[] {
  const out: LabelRange[] = [];
  for (const m of text.matchAll(LABEL)) {
    const name = m[1].slice(0, -1);
    const words = name.split(/\s+/).filter(Boolean);
    // A name: a few words that each start with a capital letter or a digit ("Alex", "Dr. Okafor", "Participant 3"), not a sentence.
    if (words.length > 4 || !words.every((w) => /^[\p{Lu}\p{N}]/u.test(w)) || NOT_NAMES.test(name.trim())) continue;
    const from = m.index! + (m[0].startsWith('\n') ? 1 : 0);
    const to = from + m[1].length + m[2].length;
    out.push({ start: index.toScalar(from), end: index.toScalar(to) });
  }
  return out;
}

/** How wide the label column is, in characters: the longest label and a space, but never so wide it crowds the words. */
export function labelColumn(labels: readonly LabelRange[]): number {
  let longest = 0;
  for (const l of labels) longest = Math.max(longest, l.end - l.start);
  return longest === 0 ? 0 : Math.min(longest + 1, 22);
}
