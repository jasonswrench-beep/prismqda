// Synthetic test data for the browser check. Nothing here is real participant text.

import fs from 'node:fs';
import path from 'node:path';

/** A short interview with an emoji, a combining accent, CJK, and right-to-left text in it. */
export const INTERVIEW = [
  'Interview with a made-up participant',
  '',
  'I felt welcome from the first day. The staff greeted me by name. \u{1F600} Cost was the hard part for me.',
  '',
  'Café owner said: Naïve plans never survive. 我感到很受欢迎。 שלום עולם is a greeting. We kept q́ apart.',
  '',
  'This is the last paragraph. Everything here is invented text for testing the screens.',
  '',
].join('\n');

export const SURVEY_CSV = [
  'pid,group,age,answer',
  'P01,a,34,I felt welcome right away and stayed for the whole season.',
  'P02,b,41,The cost was the hard part for me and my family.',
  'P03,a,29,The staff were kind and the schedule was easy to follow.',
  'P04,b,52,I could not find parking and that made every visit stressful.',
  'P05,a,38,Friends told me about it and I came because of them.',
  'P06,b,45,Nothing to add.',
  '',
].join('\n');

export const TRANSCRIPT = [
  'Interviewer: Thank you for joining me today. How did you hear about the program?',
  'Participant: A friend told me about it. I was nervous at first but felt welcome right away.',
  'Interviewer: What was the hardest part?',
  'Participant: The cost. I had to ask my family for help, and that was not easy.',
  'Interviewer: Is there anything you would change?',
  'Participant: More evening hours would help people who work late.',
  '',
].join('\n');

const WORDS = 'welcome cost staff schedule parking family friends season easy hard kind late evening nervous help change program visit stress follow'.split(' ');

/** A CSV with many one-sentence responses, to test the windowed grid. */
export function bigCsv(rows) {
  const out = ['id,text'];
  let seed = 7;
  const rand = () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  for (let i = 1; i <= rows; i++) {
    const n = 8 + Math.floor(rand() * 18);
    const words = [];
    for (let k = 0; k < n; k++) words.push(WORDS[Math.floor(rand() * WORDS.length)]);
    out.push(`R${String(i).padStart(5, '0')},Response ${i}: ${words.join(' ')}.`);
  }
  out.push('');
  return out.join('\n');
}

/** A document of many short sentences (under 1,000 units, so it opens in the document view). */
export function mediumDocument(paragraphs, sentencesEach) {
  const out = [];
  let n = 0;
  for (let p = 0; p < paragraphs; p++) {
    const sentences = [];
    for (let s = 0; s < sentencesEach; s++) {
      n++;
      const a = WORDS[(n * 3) % WORDS.length];
      const b = WORDS[(n * 7) % WORDS.length];
      const c = WORDS[(n * 11) % WORDS.length];
      sentences.push(`Sentence ${n} is about ${a} and ${b} with some ${c} too.`);
    }
    out.push(sentences.join(' '));
  }
  return out.join('\n\n') + '\n';
}

/** A short Markdown file with headings and emphasis, to see that formatting comes through. */
export const NOTES_MD = [
  '# Findings',
  '',
  'The staff were **kind** and the *schedule* was easy to follow.',
  '',
  '## Costs',
  '',
  'Cost was the hard part for most people.',
  '',
].join('\n');

/** About 380,000 characters in 900 long sentences: the biggest source that opens in the Document view. */
export function longDocument(sentences = 900, length = 420) {
  const out = [];
  for (let s = 0; s < sentences; s++) {
    const words = [];
    let n = 0;
    while (n < length - 20) {
      const w = WORDS[(s * 7 + words.length * 3) % WORDS.length];
      words.push(w);
      n += w.length + 1;
    }
    out.push(`Item ${s + 1} says ${words.join(' ')}.`);
  }
  return out.join('\n\n') + '\n';
}

export function writeFiles(dir) {
  const files = {
    notes: path.join(dir, 'notes.md'),
    long: path.join(dir, 'long-document.txt'),
    interview: path.join(dir, 'interview.txt'),
    survey: path.join(dir, 'survey.csv'),
    transcript: path.join(dir, 'focus-group.txt'),
    big: path.join(dir, 'big-survey.csv'),
    medium: path.join(dir, 'medium-document.txt'),
  };
  fs.writeFileSync(files.notes, NOTES_MD, 'utf8');
  fs.writeFileSync(files.long, longDocument(), 'utf8');
  fs.writeFileSync(files.interview, INTERVIEW, 'utf8');
  fs.writeFileSync(files.survey, SURVEY_CSV, 'utf8');
  fs.writeFileSync(files.transcript, TRANSCRIPT, 'utf8');
  fs.writeFileSync(files.big, bigCsv(15000), 'utf8');
  fs.writeFileSync(files.medium, mediumDocument(40, 20), 'utf8');
  return files;
}
