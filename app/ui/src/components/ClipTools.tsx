// Coding a stretch of a recording: marking where it starts and ends (with the player's time, or typed), a timeline that shows the coded
// stretches as stripes, and the list of what is coded. The player itself is MediaPlayer's; this asks it to move or play through the
// functions it is given, and never starts a recording on its own.

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { canDecode, decodeRecording } from '../lib/audioDecode';
import { clockText, fractionOf, isSpan, lengthText, orderedSpan, parseClock, spanProblem, spanShort, spanWords, stripeLayout, timeAtFraction } from '../lib/clips';
import { highlightColor } from '../lib/labels';
import { clipFileName, cutChannels, encodeWav, saveRateFor, toBase64 } from '../lib/waveform';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { announce } from '../state/announce';
import { clearClipDraft, markClip, reshapeClip, selectCoding } from '../state/actions';
import { appStore, focusApplyBox, setCenterFocus, useApp } from '../state/app';
import type { CodingRow, SpanShape } from '../types';
import { Notice } from './common';
import { t, tn } from '../i18n';

/** What a stretch needs to be cut out of its recording and saved: where the window reads the recording from, and what to call the file. */
export interface ClipMedia {
  url: string;
  sizeBytes: number;
  /** The document's name, which starts the name suggested for a stretch's file. */
  documentName: string;
}

const LANE = 12;

interface Group {
  key: string;
  span: SpanShape;
  codings: CodingRow[];
}

export interface ClipToolsProps {
  sourceId: number;
  /** Where the player is now, in milliseconds. */
  nowMs: number;
  /** How long the recording is, in milliseconds, when the player has said. */
  lengthMs: number | null;
  readOnly: boolean;
  /** Moves the player to a time without playing. */
  seek: (ms: number) => void;
  /** Plays from the start of a stretch and stops at its end. */
  playSpan: (startMs: number, endMs: number) => void;
  /** True when the document is the recording itself, so there is no text to give the focus back to after the code box: the tools take it. */
  standsAlone: boolean;
  /** Present when the recording can be read by the window, so a stretch can be saved as its own sound file. */
  media?: ClipMedia;
}

/** Where a clip is saved, from what was typed: a folder (ending in a slash) gets the suggested name; anything else is the file. */
export function clipTarget(typed: string, suggested: string): string {
  const path = cleanTypedPath(typed);
  return /[\\/]$/.test(path) ? `${path}${suggested}` : path;
}

/** Whether a key press belongs to a field where someone is typing. */
export function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || !el.tagName) return false;
  const tag = el.tagName.toLowerCase();
  return tag === 'input' || tag === 'textarea' || tag === 'select' || el.isContentEditable;
}

export function ClipTools(props: ClipToolsProps) {
  const { sourceId, nowMs, lengthMs, readOnly, seek, playSpan, standsAlone, media } = props;
  const data = useApp((s) => s.sourceData);
  const draft = useApp((s) => s.clipDraft);
  const selectedId = useApp((s) => s.selectedCodingId);
  const codes = useApp((s) => s.codes);
  const lineRef = useRef<HTMLDivElement>(null);

  // A document that is the recording has no text, so the focus goes back to the button that opens the code box.
  useEffect(() => {
    if (!standsAlone) return;
    setCenterFocus(() => document.querySelector<HTMLElement>('[data-testid=clip-apply]')?.focus());
    return () => setCenterFocus(null);
  }, [standsAlone]);

  const clips = useMemo(() => (data && data.id === sourceId ? data.codings.filter((c) => isSpan(c.region)) : []), [data, sourceId]);
  const groups = useMemo<Group[]>(() => {
    const by = new Map<string, Group>();
    for (const c of clips) {
      const span = c.region as SpanShape;
      const key = `${span.start_ms}-${span.end_ms}`;
      const g = by.get(key);
      if (g) g.codings.push(c);
      else by.set(key, { key, span, codings: [c] });
    }
    return [...by.values()].sort((a, b) => a.span.start_ms - b.span.start_ms || a.span.end_ms - b.span.end_ms);
  }, [clips]);

  const total = Math.max(1000, lengthMs ?? 0, nowMs, ...clips.map((c) => (c.region as SpanShape).end_ms), draft?.end_ms ?? 0, draft?.start_ms ?? 0);
  const stripes = useMemo(() => stripeLayout(clips.map((c) => ({ id: c.id, start_ms: (c.region as SpanShape).start_ms, end_ms: (c.region as SpanShape).end_ms })), total), [clips, total]);
  const lanes = Math.max(1, ...stripes.map((s) => s.lane + 1));
  const colorOf = (codeId: number): string => codes.find((c) => c.id === codeId)?.color ?? '#808080';
  const nameOf = (codeId: number): string => codes.find((c) => c.id === codeId)?.name ?? '';

  // The two typed fields follow the marks, and the marks follow what is typed once it is a time.
  const [startText, setStartText] = useState('');
  const [endText, setEndText] = useState('');
  const [typed, setTyped] = useState<string | null>(null);
  useEffect(() => {
    const tenths = (v: number | null | undefined) => (v === null || v === undefined ? false : v % 1000 >= 100);
    const fine = tenths(draft?.start_ms) || tenths(draft?.end_ms);
    setStartText(draft?.start_ms === null || draft?.start_ms === undefined ? '' : clockText(draft.start_ms, fine));
    setEndText(draft?.end_ms === null || draft?.end_ms === undefined ? '' : clockText(draft.end_ms, fine));
    setTyped(null);
  }, [draft?.start_ms, draft?.end_ms]);

  const commitTyped = (which: 'start' | 'end', text: string) => {
    if (text.trim() === '') return;
    const ms = parseClock(text);
    if (ms === null) {
      setTyped(t('Type a time such as 1:05, 1:02:03, or 65.5 (seconds).'));
      return;
    }
    setTyped(null);
    markClip(which, ms);
  };
  const onFieldKey = (e: KeyboardEvent<HTMLInputElement>, which: 'start' | 'end') => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitTyped(which, which === 'start' ? startText : endText);
    }
  };

  const problem = draft && draft.start_ms !== null && draft.end_ms !== null ? spanProblem(draft.start_ms, draft.end_ms) : null;
  const complete = !!draft && draft.start_ms !== null && draft.end_ms !== null && problem === null;
  const draftSpan = complete ? orderedSpan(draft!.start_ms as number, draft!.end_ms as number) : null;
  // A coded stretch that is chosen can be given another code.
  const chosenStretch = selectedId !== null && selectedId < 0 && clips.some((c) => c.id === selectedId);

  // A coded stretch saved as its own sound file. The recording is decoded here, only the stretch is cut out of it and written as a
  // WAV file, and the engine saves that where the person said (it never replaces a file that is there unless the person chose it in the save window).
  const [savePath, setSavePath] = useState('');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ path: string; bytes: number; namesKept: boolean; rate: number } | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveClip = async (g: Group, typedPath?: string, fromDialog = false): Promise<void> => {
    if (!media) return;
    setSaveError(null);
    setSaved(null);
    const suggested = clipFileName(media.documentName, g.span.start_ms, g.span.end_ms, g.codings.map((c) => nameOf(c.code_id)));
    let target = clipTarget(typedPath ?? savePath, suggested);
    if (target === '') {
      const picked = await browsePath('export-file', suggested, 'wav');
      if (!picked) return setSaveError(t('Type where to save the sound, for example C:\\Clips\\ (a folder) or C:\\Clips\\safety.wav'));
      target = picked;
      fromDialog = true;
    }
    setSaving(true);
    try {
      // The usual 44.1 kHz, or less for a recording so long that the whole of it at 44.1 kHz would not fit in the window's memory.
      const rate = saveRateFor(lengthMs);
      if (rate === null) throw new Error(t('This recording is too long to cut a clip from here. Link a shorter recording, or cut it in two.'));
      const sound = await decodeRecording(media.url, media.sizeBytes, rate, lengthMs);
      const cut = cutChannels(sound.channels, sound.sampleRate, g.span.start_ms, g.span.end_ms);
      if (cut.length === 0 || cut[0].length === 0) throw new Error(t('That stretch is outside the recording, so there is no sound to save. Check its times.'));
      const r = await call('export.clip', { path: target, overwrite: fromDialog, data: toBase64(encodeWav(cut, sound.sampleRate)) });
      setSaved({ path: r.path, bytes: r.bytes, namesKept: r.names_not_replaced === true, rate: sound.sampleRate });
      announce(t('Saved {path}.', { path: r.path }));
    } catch (e) {
      const m = toApiError(e).message;
      setSaveError(m);
      announce(m, 'assertive');
    } finally {
      setSaving(false);
    }
  };

  const adjust = async (g: Group, which: 'start' | 'end') => {
    const now = Math.round(nowMs);
    const next: SpanShape = which === 'start' ? { shape: 'span', start_ms: now, end_ms: g.span.end_ms } : { shape: 'span', start_ms: g.span.start_ms, end_ms: now };
    for (const c of g.codings) {
      if (!(await reshapeClip(c.id, next))) return;
    }
  };

  return (
    <div className="clip-tools" data-testid="clip-tools">
      <h3 className="bk-h3" id="clip-tools-h">{t('Code a stretch of the recording')}</h3>
      <p className="help">{t('Move the recording to where a stretch starts (play it, or type a time) and press Mark start, then to where it ends and press Mark end. Then choose a code in the Codes list, or press its favorite key. With the focus on the player or on these buttons, I marks the start and O marks the end.')}</p>

      <p className="clip-now" data-testid="clip-now">{t('The recording is at {time}.', { time: clockText(nowMs, true) })}</p>
      <div className="clip-marks" role="group" aria-labelledby="clip-tools-h">
        <Button className="img-tool" onPress={() => markClip('start', nowMs)} isDisabled={readOnly} aria-keyshortcuts="I" data-testid="clip-mark-start">{t('Mark start')}</Button>
        <Button className="img-tool" onPress={() => markClip('end', nowMs)} isDisabled={readOnly} aria-keyshortcuts="O" data-testid="clip-mark-end">{t('Mark end')}</Button>
        <div className="field clip-field">
          <label htmlFor="clip-start">{t('Starts at')}</label>
          <input
            id="clip-start"
            type="text"
            inputMode="decimal"
            value={startText}
            placeholder={t('1:05')}
            autoComplete="off"
            spellCheck={false}
            disabled={readOnly}
            onChange={(e) => setStartText(e.target.value)}
            onBlur={(e) => commitTyped('start', e.target.value)}
            onKeyDown={(e) => onFieldKey(e, 'start')}
            data-testid="clip-start-field"
          />
        </div>
        <div className="field clip-field">
          <label htmlFor="clip-end">{t('Ends at')}</label>
          <input
            id="clip-end"
            type="text"
            inputMode="decimal"
            value={endText}
            placeholder={t('1:30')}
            autoComplete="off"
            spellCheck={false}
            disabled={readOnly}
            onChange={(e) => setEndText(e.target.value)}
            onBlur={(e) => commitTyped('end', e.target.value)}
            onKeyDown={(e) => onFieldKey(e, 'end')}
            data-testid="clip-end-field"
          />
        </div>
        <Button className="img-tool" onPress={() => focusApplyBox()} isDisabled={readOnly || (!draftSpan && !chosenStretch)} data-testid="clip-apply">{t('Apply a code…')}</Button>
        <Button className="img-tool" onPress={() => draftSpan && playSpan(draftSpan.start_ms, draftSpan.end_ms)} isDisabled={!draftSpan} data-testid="clip-play-draft">{t('Play the marked stretch')}</Button>
        <Button className="img-tool" onPress={() => clearClipDraft()} isDisabled={!draft} data-testid="clip-clear">{t('Clear the marks')}</Button>
      </div>
      <p className="clip-draft" role="status" data-testid="clip-draft">
        {typed ??
          (problem
            ? problem
            : draftSpan
              ? t('Marked {span}. Choose a code to apply it.', { span: spanWords(draftSpan.start_ms, draftSpan.end_ms) })
              : draft && draft.start_ms !== null
                ? t('The stretch starts at {time}. Mark where it ends.', { time: clockText(draft.start_ms, draft.start_ms % 1000 >= 100) })
                : draft && draft.end_ms !== null
                  ? t('The stretch ends at {time}. Mark where it starts.', { time: clockText(draft.end_ms, draft.end_ms % 1000 >= 100) })
                  : t('No stretch is marked.'))}
      </p>

      <div
        className="clip-timeline"
        ref={lineRef}
        aria-hidden="true"
        style={{ height: lanes * LANE + 8 }}
        data-testid="clip-timeline"
        onClick={(e) => {
          const box = lineRef.current?.getBoundingClientRect();
          if (box && box.width > 0) seek(timeAtFraction((e.clientX - box.left) / box.width, total));
        }}
      >
        {stripes.map((s) => {
          const c = clips.find((x) => x.id === s.id)!;
          return (
            <div
              key={s.id}
              className={`clip-stripe${c.id === selectedId ? ' selected' : ''}`}
              style={{ left: `${s.left * 100}%`, width: `${s.width * 100}%`, top: 4 + s.lane * LANE, background: colorOf(c.code_id) }}
              data-testid="clip-stripe"
            />
          );
        })}
        {draft && draft.start_ms !== null ? <div className="clip-mark" style={{ left: `${fractionOf(draft.start_ms, total) * 100}%` }} data-testid="clip-mark-line" /> : null}
        {draft && draft.end_ms !== null ? <div className="clip-mark" style={{ left: `${fractionOf(draft.end_ms, total) * 100}%` }} /> : null}
        {draftSpan ? <div className="clip-draft-stripe" style={{ left: `${fractionOf(draftSpan.start_ms, total) * 100}%`, width: `${Math.max(0.004, fractionOf(draftSpan.end_ms, total) - fractionOf(draftSpan.start_ms, total)) * 100}%` }} /> : null}
        <div className="clip-playhead" style={{ left: `${fractionOf(nowMs, total) * 100}%` }} />
      </div>
      <p className="clip-scale muted" aria-hidden="true">
        <span>0:00</span>
        <span>{clockText(total)}</span>
      </p>

      <h3 className="bk-h3" id="clip-list-h">{t('Coded stretches ({count})', { count: groups.length })}</h3>
      {groups.length === 0 ? (
        <p className="muted" data-testid="clip-none">{t('No stretch of this recording is coded yet.')}</p>
      ) : (
        <ul className="clip-list" aria-labelledby="clip-list-h" data-testid="clip-list">
          {groups.map((g) => {
            const selected = g.codings.some((c) => c.id === selectedId);
            const words = spanShort(g.span.start_ms, g.span.end_ms);
            return (
              <li key={g.key} className={`clip-row${selected ? ' selected' : ''}`} data-testid="clip-row">
                <Button
                  className="clip-pick"
                  aria-pressed={selected}
                  aria-label={t('{span}, {codes}. Choose this stretch.', { span: words, codes: g.codings.map((c) => nameOf(c.code_id)).join(', ') })}
                  onPress={() => {
                    selectCoding(g.codings[0].id);
                    appStore.set({ selectionText: t('Selected: {span}', { span: spanWords(g.span.start_ms, g.span.end_ms) }) });
                    seek(g.span.start_ms);
                  }}
                >
                  <span className="clip-times">{words}</span>
                  <span className="clip-length muted">{t('{length} long', { length: lengthText(g.span.start_ms, g.span.end_ms) })}</span>
                  <span className="clip-codes">
                    {g.codings.map((c) => (
                      <span key={c.id} className="bk-chip" style={{ background: highlightColor(colorOf(c.code_id), 0.22) }}>
                        <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: colorOf(c.code_id) }} />
                        <span>{nameOf(c.code_id)}</span>
                      </span>
                    ))}
                  </span>
                </Button>
                <span className="clip-actions">
                  <Button className="img-tool" aria-label={t('Play {span}', { span: words })} onPress={() => playSpan(g.span.start_ms, g.span.end_ms)} data-testid="clip-play">{t('Play')}</Button>
                  <Button className="img-tool" aria-label={t('Make {span} start where the recording is now', { span: words })} isDisabled={readOnly} onPress={() => void adjust(g, 'start')} data-testid="clip-start-here">{t('Start here')}</Button>
                  <Button className="img-tool" aria-label={t('Make {span} end where the recording is now', { span: words })} isDisabled={readOnly} onPress={() => void adjust(g, 'end')} data-testid="clip-end-here">{t('End here')}</Button>
                  {media ? (
                    <Button className="img-tool" aria-label={t('Save {span} as a sound file', { span: words })} isDisabled={saving || !canDecode()} onPress={() => void saveClip(g)} data-testid="clip-save">{t('Save sound')}</Button>
                  ) : null}
                </span>
              </li>
            );
          })}
        </ul>
      )}
      {media && groups.length > 0 ? (
        <div className="clip-save" role="group" aria-labelledby="clip-save-h" data-testid="clip-save-box">
          <h4 className="bk-h4" id="clip-save-h">{t('Save a stretch as a sound file')}</h4>
          <div className="field">
            <label htmlFor="clip-save-path">{t('Where to save it')}</label>
            <input
              id="clip-save-path"
              type="text"
              value={savePath}
              onChange={(e) => setSavePath(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              placeholder={t('C:\\Clips\\ (a folder), or C:\\Clips\\safety.wav')}
              data-testid="clip-save-path"
            />
            <span className="help">
              {t('Press Save sound on a stretch above. End the path with a slash to give a folder, and the file is named for the document, the times, and the codes. A file that is already there is never replaced.')}
              {nativeDialogsAvailable ? ' ' + t('Leave the path empty to choose the place in the Windows save window.') : ''}
              {' ' + t('The sound is saved as a WAV file; for a video, only its sound.')}
            </span>
          </div>
          {saveError ? <Notice>{saveError}</Notice> : null}
          {saved ? (
            <Notice kind="info">
              <span data-testid="clip-saved">{t('Saved {path} ({size} KB).', { path: saved.path, size: String(Math.max(1, Math.round(saved.bytes / 1024))) })}</span>
              {saved.rate < 44100 ? ' ' + t('The recording is long, so the sound was saved at {khz} kHz to keep the window’s memory in check.', { khz: String(saved.rate / 1000) }) : ''}
              {saved.namesKept ? ' ' + t('Your list of names is being applied to exports, but a sound cannot have names taken out of it, so this file may still hold spoken names.') : ''}
            </Notice>
          ) : null}
        </div>
      ) : null}
      <p className="muted">{tn(clips.length, { one: '{count} coding on stretches of this recording.', other: '{count} codings on stretches of this recording.' })}</p>
    </div>
  );
}
