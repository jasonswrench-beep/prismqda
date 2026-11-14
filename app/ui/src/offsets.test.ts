import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OffsetIndex, scalarLength, sliceScalar, trimSpan } from './offsets';

/** Slow but obviously right: walk the string one scalar at a time. */
function oracle(text: string) {
  const scalars = Array.from(text);
  const utf16At: number[] = [0];
  for (const s of scalars) utf16At.push(utf16At[utf16At.length - 1] + s.length);
  return { scalars, utf16At };
}

const SAMPLES: Record<string, string> = {
  ascii: 'I felt welcome from the first day.',
  empty: '',
  emoji: 'Hi \u{1F600} there \u{1F44B}\u{1F3FD} friend \u{1F468}‍\u{1F469}‍\u{1F467}!',
  emojiOnly: '\u{1F600}\u{1F601}\u{1F602}',
  // "e" + combining acute, and a letter with no precomposed form ("q" + combining acute).
  combining: 'café and q́uick, ạ̈ too',
  precomposed: 'café über',
  cjk: '我感到很受欢迎。 今天很好',
  // Characters above U+FFFF in a CJK block (CJK Extension B).
  cjkExtB: '\u{20BB7}野家 \u{2070E}\u{29E3D}',
  hebrew: 'אני הרגשתי מוקבל בחום',
  arabic: 'شعرت بالترحيب من أول يوم',
  // Adlam is a right-to-left script whose letters are above U+FFFF.
  adlam: '\u{1E900}\u{1E901}\u{1E902} \u{1E922}\u{1E923}',
  mixed: 'abc אני \u{1F600} 我 é \u{20BB7} شعر xyz\n\nNext paragraph \u{1F680}.',
  newlines: 'one\ntwo\n\nthree \u{1F600}\nfour',
};

for (const [name, text] of Object.entries(SAMPLES)) {
  test(`scalar and UTF-16 positions round-trip: ${name}`, () => {
    const idx = new OffsetIndex(text);
    const { scalars, utf16At } = oracle(text);
    assert.equal(idx.scalarLength, scalars.length);
    assert.equal(idx.utf16Length, text.length);
    assert.equal(scalarLength(text), scalars.length);
    for (let s = 0; s <= scalars.length; s++) {
      assert.equal(idx.toUtf16(s), utf16At[s], `scalar ${s} to UTF-16`);
      assert.equal(idx.toScalar(utf16At[s]), s, `UTF-16 ${utf16At[s]} to scalar`);
      assert.equal(idx.toScalar(utf16At[s], 'up'), s);
    }
  });
}

test('a character above U+FFFF is one scalar and two UTF-16 units', () => {
  const text = 'a\u{1F600}b';
  assert.equal(text.length, 4);
  const idx = new OffsetIndex(text);
  assert.equal(idx.scalarLength, 3);
  assert.equal(idx.toUtf16(2), 3);
  assert.equal(idx.toScalar(3), 2);
  assert.equal(sliceScalar(text, idx, 1, 2), '\u{1F600}');
  assert.equal(sliceScalar(text, idx, 1, 3), '\u{1F600}b');
});

test('a position between the two halves of a character snaps to its start or its end', () => {
  const text = 'a\u{1F600}b';
  const idx = new OffsetIndex(text);
  // UTF-16 index 2 is between the high and low halves of the emoji.
  assert.equal(idx.toScalar(2, 'down'), 1);
  assert.equal(idx.toScalar(2, 'up'), 2);
  assert.equal(idx.toScalar(2), 1);
});

test('positions outside the text are held inside it', () => {
  const idx = new OffsetIndex('a\u{1F600}b');
  assert.equal(idx.toScalar(-5), 0);
  assert.equal(idx.toScalar(500), 3);
  assert.equal(idx.toUtf16(-1), 0);
  assert.equal(idx.toUtf16(99), 4);
  assert.equal(idx.toUtf16(Number.NaN), 0);
});

test('a text without characters above U+FFFF is plain: both counts agree', () => {
  assert.equal(new OffsetIndex(SAMPLES.combining).isPlain, true);
  assert.equal(new OffsetIndex(SAMPLES.cjk).isPlain, true);
  assert.equal(new OffsetIndex(SAMPLES.hebrew).isPlain, true);
  assert.equal(new OffsetIndex(SAMPLES.emoji).isPlain, false);
});

test('combining marks count as scalars of their own, like the engine counts them', () => {
  const text = 'é';
  const idx = new OffsetIndex(text);
  assert.equal(idx.scalarLength, 2);
  assert.equal(text.normalize('NFC').length, 1, 'NFC would join them, but a letter like q has no joined form');
  assert.equal(new OffsetIndex('q́').scalarLength, 2);
});

test('a lone surrogate is one scalar and does not break the count', () => {
  const text = 'a\ud83dz\ude00b';
  const idx = new OffsetIndex(text);
  assert.equal(idx.scalarLength, 5);
  assert.equal(idx.toUtf16(3), 3);
});

test('a long text with many emoji converts quickly and correctly', () => {
  const unit = 'word \u{1F600} more é text. ';
  const text = unit.repeat(40_000);
  const idx = new OffsetIndex(text);
  assert.equal(idx.scalarLength, Array.from(unit).length * 40_000);
  const started = Date.now();
  let checksum = 0;
  for (let s = 0; s <= idx.scalarLength; s += 97) {
    const u = idx.toUtf16(s);
    checksum += idx.toScalar(u) === s ? 0 : 1;
  }
  assert.equal(checksum, 0);
  assert.ok(Date.now() - started < 2000, 'conversion should be fast');
});

test('random mixed text round-trips on every boundary', () => {
  let seed = 12345;
  const rand = () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const pool = ['a', 'b', ' ', '\n', 'é', 'é', '我', 'א', 'ش', '\u{1F600}', '\u{20BB7}', '\u{1E900}', '\u{1F44B}\u{1F3FD}'];
  for (let round = 0; round < 50; round++) {
    let text = '';
    const n = Math.floor(rand() * 60);
    for (let i = 0; i < n; i++) text += pool[Math.floor(rand() * pool.length)];
    const idx = new OffsetIndex(text);
    const { scalars, utf16At } = oracle(text);
    assert.equal(idx.scalarLength, scalars.length);
    for (let s = 0; s <= scalars.length; s++) {
      assert.equal(idx.toUtf16(s), utf16At[s]);
      assert.equal(idx.toScalar(utf16At[s]), s);
    }
  }
});

test('trimSpan drops spaces and line breaks at both ends and counts in scalars', () => {
  const text = 'He said \u{1F600} felt welcome \n there';
  const idx = new OffsetIndex(text);
  const words = Array.from(text);
  const start = words.indexOf(' ', 8) ; // the space after the emoji
  const end = words.indexOf('\n') + 1;
  const trimmed = trimSpan(text, idx, start, end);
  assert.ok(trimmed);
  assert.equal(sliceScalar(text, idx, trimmed.start, trimmed.end), 'felt welcome');
  assert.equal(trimSpan(text, idx, 7, 8), null, 'only a space');
  const emoji = trimSpan(text, idx, 7, 10);
  assert.ok(emoji);
  assert.equal(sliceScalar(text, idx, emoji.start, emoji.end), '\u{1F600}');
});

test('trimSpan keeps a span that is already tight', () => {
  const text = '我感到很受欢迎';
  const idx = new OffsetIndex(text);
  assert.deepEqual(trimSpan(text, idx, 1, 4), { start: 1, end: 4 });
});
