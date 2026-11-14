// The sound of a recording as the window works with it: the peaks that are drawn as a waveform, a stretch cut out of the samples, and a
// WAV file made of it. Everything here is a plain function of numbers, so it is checked without a browser (waveform.test.ts); the
// decoding of a recording into samples is in audioDecode.ts.

import { t } from '../i18n';

/** The largest recording (as a file) the window will decode to draw a waveform or to cut a stretch from. Decoding holds the whole sound in memory. */
export const MAX_DECODE_BYTES = 250 * 1024 * 1024;

/** The most memory, in bytes, the window will let a decoded recording take. The sound is held as 32-bit numbers for each channel, and a recording's channels are not known beforehand, so two are assumed. */
export const MAX_DECODED_BYTES = 700 * 1024 * 1024;

/** What a recording of `durationMs` takes in memory once decoded at `rate`, with two channels. */
export function decodedBytes(durationMs: number, rate: number): number {
  return Math.max(0, durationMs / 1000) * rate * 2 * 4;
}

/**
 * The sample rate to cut a clip at: the usual 44.1 kHz, or a lower one when the recording is so long that the whole of it decoded at 44.1 kHz would
 * not fit in the memory the window allows. `null` when even the lowest does not fit. A recording of unknown length is taken as it comes.
 */
export function saveRateFor(durationMs: number | null): number | null {
  if (durationMs === null || !Number.isFinite(durationMs)) return 44100;
  for (const rate of [44100, 22050, 16000, 11025]) if (decodedBytes(durationMs, rate) <= MAX_DECODED_BYTES) return rate;
  return null;
}

/** How many bars a waveform is drawn with. */
export const WAVEFORM_BUCKETS = 1200;

/**
 * The loudness of each of `buckets` equal parts of a sound, from 0 (silence) to 1: the largest sample, in any channel, in the part.
 * A sound shorter than the number of parts gives fewer, and no sound gives none.
 */
export function peaksOf(channels: Float32Array[], buckets: number): Float32Array {
  const length = channels.reduce((n, c) => Math.max(n, c.length), 0);
  if (length === 0 || buckets < 1) return new Float32Array(0);
  const parts = Math.min(buckets, length);
  const out = new Float32Array(parts);
  for (let b = 0; b < parts; b++) {
    const from = Math.floor((b * length) / parts);
    const to = Math.max(from + 1, Math.floor(((b + 1) * length) / parts));
    let top = 0;
    for (const c of channels) {
      const end = Math.min(to, c.length);
      for (let i = from; i < end; i++) {
        const v = Math.abs(c[i]);
        if (v > top) top = v;
      }
    }
    out[b] = Math.min(1, top);
  }
  return out;
}

/** The SVG path that draws peaks as bars mirrored about the middle of a box `width` by `height`; a bar is at least a pixel tall so silence still shows a line. */
export function waveformPath(peaks: Float32Array, width: number, height: number): string {
  if (peaks.length === 0) return '';
  const step = width / peaks.length;
  const mid = height / 2;
  let d = '';
  for (let i = 0; i < peaks.length; i++) {
    const x = (i + 0.5) * step;
    const half = Math.max(0.5, peaks[i] * mid * 0.94);
    d += `M${x.toFixed(2)} ${(mid - half).toFixed(2)}V${(mid + half).toFixed(2)}`;
  }
  return d;
}

/** The samples of each channel from `startMs` to `endMs`, clamped to the sound. */
export function cutChannels(channels: Float32Array[], sampleRate: number, startMs: number, endMs: number): Float32Array[] {
  return channels.map((c) => {
    const from = Math.max(0, Math.min(c.length, Math.round((startMs / 1000) * sampleRate)));
    const to = Math.max(from, Math.min(c.length, Math.round((endMs / 1000) * sampleRate)));
    return c.slice(from, to);
  });
}

/** A WAV file (16-bit samples, the channels interleaved) of the channels. Samples beyond -1 and 1 are held at the ends. */
export function encodeWav(channels: Float32Array[], sampleRate: number): Uint8Array {
  const count = channels.length;
  const frames = count === 0 ? 0 : channels[0].length;
  const dataBytes = frames * count * 2;
  const out = new Uint8Array(44 + dataBytes);
  const view = new DataView(out.buffer);
  const text = (at: number, s: string) => {
    for (let i = 0; i < s.length; i++) out[at + i] = s.charCodeAt(i);
  };
  text(0, 'RIFF');
  view.setUint32(4, 36 + dataBytes, true);
  text(8, 'WAVE');
  text(12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, count, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * count * 2, true);
  view.setUint16(32, count * 2, true);
  view.setUint16(34, 16, true);
  text(36, 'data');
  view.setUint32(40, dataBytes, true);
  let at = 44;
  for (let i = 0; i < frames; i++) {
    for (let c = 0; c < count; c++) {
      const v = Math.max(-1, Math.min(1, channels[c][i]));
      view.setInt16(at, Math.round(v < 0 ? v * 32768 : v * 32767), true);
      at += 2;
    }
  }
  return out;
}

/** Bytes as base 64 (the form the engine is sent a file in), in pieces so a long file does not overflow the call. */
export function toBase64(bytes: Uint8Array): string {
  let binary = '';
  const piece = 0x8000;
  for (let i = 0; i < bytes.length; i += piece) binary += String.fromCharCode(...bytes.subarray(i, i + piece));
  return btoa(binary);
}

/** A time for a file name: `1m05s`, or `1h02m03s` past an hour (no colon, which Windows does not allow in a name). */
export function nameTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}h${two(m)}m${two(s)}s` : `${m}m${two(s)}s`;
}

/** The name suggested for a clip's file: the document, the times, and the codes; characters Windows does not allow become dashes, and it stays short. */
export function clipFileName(document: string, startMs: number, endMs: number, codes: string[]): string {
  const clean = (s: string) => s.replace(/[\\/:*?"<>|\r\n\t]/g, '-').replace(/\s+/g, ' ').trim();
  const base = [clean(document) || t('Recording'), `${nameTime(startMs)}-${nameTime(endMs)}`, ...codes.map(clean).filter(Boolean)].join(' ');
  return `${base.slice(0, 120).trim()}.wav`;
}
