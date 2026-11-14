import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MAX_DECODED_BYTES, clipFileName, cutChannels, decodedBytes, encodeWav, nameTime, peaksOf, saveRateFor, toBase64, waveformPath } from './waveform';

test('the loudness of each part is the largest sample of any channel in it', () => {
  const left = Float32Array.from([0, 0.5, 0, 0, -0.25, 0, 0, 0]);
  const right = Float32Array.from([0, 0, 0, 0.75, 0, 0, 0, -1]);
  const p = peaksOf([left, right], 4);
  assert.deepEqual(Array.from(p), [0.5, 0.75, 0.25, 1]);
  assert.equal(peaksOf([left], 100).length, 8, 'a short sound has a part for each sample, not more');
  assert.equal(peaksOf([], 10).length, 0, 'no sound has no peaks');
  assert.equal(peaksOf([new Float32Array(0)], 10).length, 0);
  assert.equal(peaksOf([Float32Array.from([3, -4])], 2)[1], 1, 'a value beyond 1 is held at 1');
});

test('a waveform is a path of mirrored bars with a line at least for silence', () => {
  const d = waveformPath(Float32Array.from([0, 1]), 100, 40);
  const bars = d.split('M').filter(Boolean);
  assert.equal(bars.length, 2);
  assert.match(bars[0], /^25\.00 19\.50V20\.50$/, 'silence is a line of one pixel');
  assert.match(bars[1], /^75\.00 1\.20V38\.80$/, 'the loudest part nearly fills the height');
  assert.equal(waveformPath(new Float32Array(0), 100, 40), '');
});

test('a stretch is cut by its times, clamped to the sound', () => {
  const ch = [Float32Array.from({ length: 8000 }, (_, i) => i)];
  const cut = cutChannels(ch, 8000, 250, 500);
  assert.equal(cut[0].length, 2000);
  assert.equal(cut[0][0], 2000, 'it starts a quarter of a second in');
  assert.equal(cutChannels(ch, 8000, -500, 100)[0].length, 800, 'a start before the sound is the start');
  assert.equal(cutChannels(ch, 8000, 900, 5000)[0].length, 800, 'an end after the sound is the end');
  assert.equal(cutChannels(ch, 8000, 700, 100)[0].length, 0, 'an end before the start is nothing');
});

test('a WAV file has the header of what it holds and the samples in 16 bits, interleaved', () => {
  const left = Float32Array.from([0, 1, -1, 0.5]);
  const right = Float32Array.from([0, -1, 2, -0.5]);
  const bytes = encodeWav([left, right], 22050);
  const v = new DataView(bytes.buffer);
  const word = (at: number) => String.fromCharCode(...bytes.subarray(at, at + 4));
  assert.deepEqual([word(0), word(8), word(12), word(36)], ['RIFF', 'WAVE', 'fmt ', 'data']);
  assert.equal(bytes.length, 44 + 4 * 2 * 2);
  assert.equal(v.getUint32(4, true), bytes.length - 8, 'the size in the header');
  assert.deepEqual([v.getUint16(20, true), v.getUint16(22, true), v.getUint32(24, true), v.getUint32(28, true), v.getUint16(32, true), v.getUint16(34, true)], [1, 2, 22050, 22050 * 4, 4, 16]);
  assert.equal(v.getUint32(40, true), 16, 'the size of the samples');
  const sample = (frame: number, channel: number) => v.getInt16(44 + (frame * 2 + channel) * 2, true);
  assert.deepEqual([sample(1, 0), sample(1, 1), sample(2, 0), sample(2, 1)], [32767, -32768, -32768, 32767], 'full scale, and beyond it held at the ends');
  assert.equal(sample(3, 0), 16384);
  assert.equal(encodeWav([], 8000).length, 44, 'no channels is a header');
});

test('bytes are written as base 64, a long file too', () => {
  assert.equal(toBase64(Uint8Array.from([104, 105])), 'aGk=');
  const long = new Uint8Array(200_000).fill(65);
  assert.equal(atob(toBase64(long)).length, 200_000);
});

test('a clip is named for its document, its times, and its codes, in characters a file name may hold', () => {
  assert.equal(nameTime(65_500), '1m05s');
  assert.equal(nameTime(3_723_000), '1h02m03s');
  assert.equal(clipFileName('Interview: Sarah/Jones', 65_000, 92_000, ['Safety', 'Cost']), 'Interview- Sarah-Jones 1m05s-1m32s Safety Cost.wav');
  assert.equal(clipFileName('', 0, 1000, []), 'Recording 0m00s-0m01s.wav');
  assert.ok(clipFileName('x'.repeat(500), 0, 1000, []).length <= 124, 'a long name is cut');
});

test('a clip is cut at the usual rate, or at a lower one when the whole recording would not fit in memory', () => {
  assert.equal(saveRateFor(null), 44100, 'a recording of unknown length is taken as it comes');
  assert.equal(saveRateFor(12_000), 44100);
  assert.equal(saveRateFor(30 * 60_000), 44100, 'half an hour fits at the usual rate');
  assert.equal(saveRateFor(60 * 60_000), 22050, 'an hour does not, but fits at half the rate');
  assert.equal(saveRateFor(90 * 60_000), 16000);
  assert.equal(saveRateFor(120 * 60_000), 11025);
  assert.equal(saveRateFor(5 * 3_600_000), null, 'five hours fits at no rate');
  for (const minutes of [10, 45, 80, 130]) {
    const rate = saveRateFor(minutes * 60_000);
    assert.ok(rate === null || decodedBytes(minutes * 60_000, rate) <= MAX_DECODED_BYTES, `${minutes} minutes at ${rate}`);
  }
  assert.equal(decodedBytes(1000, 8000), 64_000, 'a second of two channels of 32-bit numbers at 8 kHz');
});
