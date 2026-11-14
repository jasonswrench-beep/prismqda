// Turning a recording into samples, in the window, with the browser's own decoder (so every kind of file the window can play can be drawn
// and cut: WAV, MP3, AAC, Ogg, FLAC, and the sound of a video). Nothing is sent anywhere: the recording is read from the address the
// window plays it from. The sound is held in memory while it is used, so a recording larger than MAX_DECODE_BYTES is refused.

import { MAX_DECODE_BYTES, MAX_DECODED_BYTES, decodedBytes } from './waveform';
import { t } from '../i18n';

export interface Decoded {
  /** One array of samples, from -1 to 1, for each channel. */
  channels: Float32Array[];
  sampleRate: number;
  /** How long the sound is, in milliseconds. */
  lengthMs: number;
}

/** True when this window can decode sound at all (a plain page in a test may not). */
export function canDecode(): boolean {
  return typeof window !== 'undefined' && (typeof window.OfflineAudioContext === 'function' || typeof (window as unknown as { webkitOfflineAudioContext?: unknown }).webkitOfflineAudioContext === 'function');
}

/** How much of a recording is asked for at a time. The window answers at most this much to one request, whatever was asked for. */
const READ_CHUNK = 4 * 1024 * 1024;

/**
 * All of the recording at `url`, which is `sizeBytes` long. A request with no range is answered with the first part of a large recording (status
 * 206), not the whole of it, so a large one is read part by part. An answer of the whole file (status 200) is taken as it is.
 */
export async function readAll(url: string, sizeBytes: number): Promise<ArrayBuffer> {
  const parts: Uint8Array[] = [];
  let got = 0;
  while (got < sizeBytes) {
    const response = await fetch(url, { headers: { Range: `bytes=${got}-${got + READ_CHUNK - 1}` } });
    if (response.status === 200) return response.arrayBuffer();
    if (response.status !== 206) throw new Error(String(response.status));
    const part = new Uint8Array(await response.arrayBuffer());
    if (part.length === 0) break;
    parts.push(part);
    got += part.length;
  }
  const all = new Uint8Array(got);
  let at = 0;
  for (const part of parts) {
    all.set(part, at);
    at += part.length;
  }
  return all.buffer;
}

/**
 * Reads the recording at `url` and decodes it at `sampleRate` (the decoder changes the rate). `sizeBytes` is the size the project knows the file
 * to be, and `durationMs` how long it is when that is known, so a recording too large to hold in memory is refused before anything is read. Throws an Error whose message is for the person.
 */
export async function decodeRecording(url: string, sizeBytes: number, sampleRate: number, durationMs: number | null = null): Promise<Decoded> {
  if (!canDecode()) throw new Error(t('This window cannot read the sound of a recording.'));
  if (sizeBytes > MAX_DECODE_BYTES) throw new Error(t('This recording is too large to read into memory here ({mb} MB; the limit is {limit} MB).', { mb: String(Math.round(sizeBytes / 1048576)), limit: String(Math.round(MAX_DECODE_BYTES / 1048576)) }));
  if (durationMs !== null && Number.isFinite(durationMs) && decodedBytes(durationMs, sampleRate) > MAX_DECODED_BYTES) {
    throw new Error(t('This recording is too long to read into memory here ({hours} hours). Link a shorter recording, or cut it in two.', { hours: (durationMs / 3_600_000).toFixed(1) }));
  }
  let bytes: ArrayBuffer;
  try {
    bytes = await readAll(url, sizeBytes);
  } catch {
    throw new Error(t('The recording could not be read. Is the file still where it was?'));
  }
  const Ctx = window.OfflineAudioContext ?? (window as unknown as { webkitOfflineAudioContext: typeof OfflineAudioContext }).webkitOfflineAudioContext;
  try {
    const buffer = await new Ctx(1, 1, sampleRate).decodeAudioData(bytes);
    const channels: Float32Array[] = [];
    for (let c = 0; c < buffer.numberOfChannels; c++) channels.push(buffer.getChannelData(c));
    return { channels, sampleRate: buffer.sampleRate, lengthMs: Math.round(buffer.duration * 1000) };
  } catch {
    throw new Error(t('The sound of this recording could not be decoded here.'));
  }
}
