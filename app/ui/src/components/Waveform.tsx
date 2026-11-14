// The waveform of a recording: how loud it is from the start to the end, with the coded stretches behind it and the place the player is at.
// It is drawn only when the person asks, because the whole sound has to be decoded to draw it (with the browser's own decoder, at a low
// sample rate, and then dropped: only the peaks are kept). It is a slider: a click, or the arrow keys, move the recording without playing it.

import { useMemo, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { Button } from 'react-aria-components';
import { canDecode, decodeRecording } from '../lib/audioDecode';
import { clockText, fractionOf, isSpan, timeAtFraction } from '../lib/clips';
import { peaksOf, waveformPath, WAVEFORM_BUCKETS } from '../lib/waveform';
import { useApp } from '../state/app';
import type { SpanShape } from '../types';
import { Notice } from './common';
import { t } from '../i18n';

const W = 1200;
const H = 80;
/** The sample rate the sound is decoded at to draw it: far below hearing quality, because only its loudness over time is wanted. */
const DRAW_RATE = 8000;

interface Drawn {
  peaks: Float32Array;
  lengthMs: number;
}

/** What has been drawn, by recording and by the version of the file that was linked, so a recording is decoded once however often the page is left and come back to. */
const DRAWN = new Map<string, Drawn>();

export interface WaveformProps {
  sourceId: number;
  /** Where the window reads the recording from. */
  url: string;
  sizeBytes: number;
  /** Changes when another file is linked, so an old drawing is not shown for it. */
  version: string;
  fileName: string;
  /** How long the recording is, in milliseconds, when that is known (it decides whether the window can hold the sound). */
  durationMs?: number | null;
  /** Where the player is now, in milliseconds. */
  nowMs: number;
  /** Moves the player to a time without playing. */
  seek: (ms: number) => void;
}

export function Waveform(props: WaveformProps) {
  const { sourceId, url, sizeBytes, version, fileName, durationMs = null, nowMs, seek } = props;
  const key = `${sourceId}|${version}`;
  const data = useApp((s) => s.sourceData);
  const codes = useApp((s) => s.codes);
  const [drawn, setDrawn] = useState<Drawn | null>(() => DRAWN.get(key) ?? null);
  const [loading, setLoading] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const shown = drawn && DRAWN.get(key) === drawn ? drawn : DRAWN.get(key) ?? null;

  const stretches = useMemo(() => (data && data.id === sourceId ? data.codings.filter((c) => isSpan(c.region)) : []), [data, sourceId]);
  const path = useMemo(() => (shown ? waveformPath(shown.peaks, W, H) : ''), [shown]);
  const loudest = useMemo(() => {
    if (!shown || shown.peaks.length === 0) return null;
    let at = 0;
    for (let i = 1; i < shown.peaks.length; i++) if (shown.peaks[i] > shown.peaks[at]) at = i;
    return { ms: Math.round(((at + 0.5) / shown.peaks.length) * shown.lengthMs), level: shown.peaks[at] };
  }, [shown]);

  const show = async () => {
    setLoading(true);
    setProblem(null);
    try {
      const decoded = await decodeRecording(url, sizeBytes, DRAW_RATE, durationMs);
      const made: Drawn = { peaks: peaksOf(decoded.channels, WAVEFORM_BUCKETS), lengthMs: decoded.lengthMs };
      DRAWN.set(key, made);
      setDrawn(made);
    } catch (e) {
      setProblem(e instanceof Error ? e.message : t('The waveform could not be drawn.'));
    } finally {
      setLoading(false);
    }
  };

  if (!shown) {
    return (
      <div className="bk-waveform" data-testid="waveform-box">
        <Button className="img-tool" onPress={() => void show()} isDisabled={loading || !canDecode()} data-testid="waveform-show">
          {loading ? t('Drawing the waveform…') : t('Show the waveform')}
        </Button>
        {!canDecode() ? <span className="muted"> {t('This window cannot read the sound of a recording.')}</span> : null}
        {problem ? <Notice>{problem}</Notice> : null}
      </div>
    );
  }

  const total = Math.max(1, shown.lengthMs);
  const colorOf = (id: number) => codes.find((c) => c.id === id)?.color ?? '#808080';
  const move = (ms: number) => seek(Math.max(0, Math.min(total, Math.round(ms))));
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 30000 : 5000;
    if (e.key === 'ArrowRight' || e.key === 'ArrowUp') move(nowMs + step);
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') move(nowMs - step);
    else if (e.key === 'Home') move(0);
    else if (e.key === 'End') move(total);
    else return;
    e.preventDefault();
  };
  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    if (box.width > 0) move(timeAtFraction((e.clientX - box.left) / box.width, total));
  };

  return (
    <div className="bk-waveform" data-testid="waveform-box">
      <div
        className="bk-wave"
        role="slider"
        tabIndex={0}
        aria-label={t('Position in {file}, shown on its waveform. Arrow keys move by 5 seconds, with Shift by 30.', { file: fileName })}
        aria-valuemin={0}
        aria-valuemax={total}
        aria-valuenow={Math.min(total, Math.round(nowMs))}
        aria-valuetext={t('{now} of {total}', { now: clockText(nowMs), total: clockText(total) })}
        onKeyDown={onKey}
        onClick={onClick}
        data-testid="waveform"
      >
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true" focusable="false">
          {stretches.map((c) => {
            const s = c.region as SpanShape;
            const left = fractionOf(s.start_ms, total);
            return <rect key={c.id} x={left * W} y={0} width={Math.max(2, (fractionOf(s.end_ms, total) - left) * W)} height={H} fill={colorOf(c.code_id)} opacity={0.28} data-testid="waveform-stretch" />;
          })}
          <path d={path} className="bk-wave-line" fill="none" strokeWidth={1} vectorEffect="non-scaling-stroke" />
          <line x1={fractionOf(nowMs, total) * W} x2={fractionOf(nowMs, total) * W} y1={0} y2={H} className="bk-wave-head" strokeWidth={2} vectorEffect="non-scaling-stroke" />
        </svg>
      </div>
      <p className="clip-scale muted" aria-hidden="true">
        <span>0:00</span>
        <span>{clockText(total)}</span>
      </p>
      <p className="muted" data-testid="waveform-note">
        {loudest && loudest.level >= 0.02 ? t('The loudest part is at {time}.', { time: clockText(loudest.ms) }) : t('This recording is quiet throughout.')}{' '}
        {stretches.length > 0 ? t('The shaded parts are the stretches coded on it.') : ''}
      </p>
    </div>
  );
}
