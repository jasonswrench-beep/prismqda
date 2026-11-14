// The recording of a document: a player above the text, the turn being spoken, and the ways between the recording and the words.
// PrismQDA plays a recording only when the person presses Play (on the player, or on "Play from the caret"). Nothing here
// starts a recording by itself.

import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type KeyboardEvent, type RefObject } from 'react';
import { Button, Checkbox } from 'react-aria-components';
import { call, mediaUrl, toApiError } from '../bridge';
import { formatClock, timeForOffset, timedUnits, turnLabel, unitAt } from '../lib/media';
import { markClip } from '../state/actions';
import { sliceScalar } from '../offsets';
import { browseManyFiles, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { bumpData } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp, type SourceData } from '../state/app';
import type { MediaRow } from '../types';
import { ClipTools, isTypingTarget } from './ClipTools';
import { Notice } from './common';
import { Waveform } from './Waveform';
import { t } from '../i18n';

const MAX_TURNS_LISTED = 1000;

export function MediaPlayer(props: { data: SourceData; isTranscript: boolean }) {
  const { data } = props;
  const version = useApp((s) => s.dataVersion);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const [media, setMedia] = useState<MediaRow | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [path, setPath] = useState('');
  const [follow, setFollow] = useState(false);
  const [nowId, setNowId] = useState<number | null>(null);
  const [length, setLength] = useState<number | null>(null);
  const [nowMs, setNowMs] = useState(0);
  /** When a stretch is being played from a button, the time (in milliseconds) it stops at. */
  const playUntil = useRef<number | null>(null);
  const command = useApp((s) => s.clipCommand);
  const player = useRef<HTMLMediaElement>(null);
  const followRef = useRef(follow);
  followRef.current = follow;
  const lastShown = useRef<number | null>(null);

  const timed = useMemo(() => timedUnits(data.units), [data.units]);
  const now = timed.find((u) => u.id === nowId) ?? null;

  // The recording of the open document is asked for again when the document or the project's data changes.
  useEffect(() => {
    let live = true;
    setNowId(null);
    setLength(null);
    setNowMs(0);
    playUntil.current = null;
    lastShown.current = null;
    call('media.get', { source_id: data.id })
      .then((m) => {
        if (!live) return;
        setMedia(m);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [data.id, version]);

  const wordsOf = useCallback((start: number, end: number) => sliceScalar(data.text, data.index, start, end), [data.text, data.index]);

  /** The player moved: say which turn is being spoken, and scroll the text to it when asked to. */
  const moved = () => {
    const el = player.current;
    if (!el) return;
    setNowMs(el.currentTime * 1000);
    // A stretch played from a button stops where it ends.
    if (playUntil.current !== null && el.currentTime * 1000 >= playUntil.current) {
      playUntil.current = null;
      el.pause();
    }
    const unit = unitAt(timed, el.currentTime * 1000);
    setNowId(unit ? unit.id : null);
    if (unit && followRef.current && lastShown.current !== unit.id) {
      lastShown.current = unit.id;
      appStore.set({ mediaFollow: { sourceId: data.id, offset: unit.start, tick: Date.now() } });
    }
  };

  const link = async (e?: FormEvent) => {
    e?.preventDefault();
    const typed = cleanTypedPath(path);
    if (typed === '') {
      setError(t('Type the full path of the recording, for example C:\\Recordings\\interview-1.mp3'));
      return;
    }
    try {
      const row = await call('media.link', { source_id: data.id, path: typed });
      setMedia(row);
      setError(null);
      setPath('');
      bumpData();
      announce(t('{file_name} is linked to this document.', { file_name: row.file_name }));
    } catch (err) {
      const m = toApiError(err).message;
      setError(m);
      announce(m, 'assertive');
    }
  };

  const choose = async () => {
    const files = await browseManyFiles('media');
    if (files[0]) setPath(files[0]);
  };

  const unlink = async () => {
    try {
      await call('media.unlink', { source_id: data.id });
      setMedia(null);
      bumpData();
      announce(t('The recording is no longer linked to this document. The recording itself was not touched.'));
    } catch (err) {
      setError(toApiError(err).message);
    }
  };

  const moveTo = (ms: number, play: boolean) => {
    const el = player.current;
    if (!el) return;
    playUntil.current = null;
    el.currentTime = ms / 1000;
    setNowMs(ms);
    if (play) void el.play();
  };

  /** Plays from the start of a stretch to its end, because a button was pressed (never by itself). */
  const playSpan = (startMs: number, endMs: number) => {
    const el = player.current;
    if (!el) return;
    el.currentTime = startMs / 1000;
    setNowMs(startMs);
    playUntil.current = endMs;
    void el.play();
  };

  // Something outside the player asked it to move or to play a stretch (a chosen stretch in a list, or its details). Only a request made
  // a moment ago counts, so coming back to this tab later does not move the recording.
  useEffect(() => {
    if (!command || command.sourceId !== data.id || Date.now() - command.tick > 3000) return;
    if (command.kind === 'play') playSpan(command.start_ms, command.end_ms);
    else moveTo(command.start_ms, false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [command?.tick, media]);

  /** I marks the start of a stretch and O marks its end, while the focus is in the recording's section and not in a field. */
  const onKeys = (e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.metaKey || e.altKey || isTypingTarget(e.target) || readOnly || !media?.found) return;
    const key = e.key.toLowerCase();
    if (key !== 'i' && key !== 'o') return;
    const el = player.current;
    if (!el) return;
    e.preventDefault();
    markClip(key === 'i' ? 'start' : 'end', el.currentTime * 1000);
  };

  if (media === undefined && error === null) return null;
  if (!media && !props.isTranscript) return error ? <Notice>{error}</Notice> : null;

  // A transcript with no recording yet offers to link one.
  if (!media) {
    return (
      <section className="bk-media" aria-label={t('Recording')} data-testid="media-player">
        <details>
          <summary>{timed.length > 0 ? t('Recording: none linked. This transcript has times, so it can follow one.') : t('Recording: none linked')}</summary>
          <LinkForm path={path} setPath={setPath} onLink={link} onChoose={choose} disabled={readOnly} />
          {error ? <Notice>{error}</Notice> : null}
        </details>
      </section>
    );
  }

  const url = `${mediaUrl(data.id)}?t=${encodeURIComponent(media.created_at)}`;
  // How long the recording is, in milliseconds, as far as it is known: a player reports Infinity for a stream with no length in it, and then the length the captions give is used.
  const knownMs = length !== null && Number.isFinite(length) ? length * 1000 : media.duration_ms;
  const turns = timed.slice(0, MAX_TURNS_LISTED);
  const mb = media.size_bytes / (1024 * 1024);
  const label = t('Recording: {file}', { file: media.file_name });
  const unlinkButton = (
    <Button onPress={() => void unlink()} isDisabled={readOnly} data-testid="media-unlink">{t('Unlink this recording')}</Button>
  );
  return (
    <section className="bk-media" aria-label={t('Recording')} data-testid="media-player" onKeyDown={onKeys}>
      <p className="bk-media-line">
        <strong data-testid="media-name">{media.file_name}</strong>
        <span className="muted">
          {' '}
          {media.kind === 'video' ? t('Video') : t('Audio')}, {mb >= 1 ? t('{mb} MB', { mb: mb.toFixed(1) }) : t('{value} KB', { value: Math.max(1, Math.round(media.size_bytes / 1024)) })}
          {length !== null && Number.isFinite(length) ? t(', {time} long', { time: formatClock(length * 1000) }) : media.duration_ms !== null ? t(', about {time} long', { time: formatClock(media.duration_ms) }) : ''}
        </span>
      </p>
      {!media.found ? (
        <>
          <Notice kind="warning">
            <span data-testid="media-missing">{t('The recording {file_name} is not at {path} any more. Link it again from its new place.', { file_name: media.file_name, path: media.path })}</span>
          </Notice>
          <LinkForm path={path} setPath={setPath} onLink={link} onChoose={choose} disabled={readOnly} />
          <div className="actions">{unlinkButton}</div>
        </>
      ) : (
        <>
          {!media.unchanged ? (
            <Notice kind="warning">
              <span data-testid="media-changed">{t('The file at that place is not the size it was when it was linked, so it may have been replaced. Check that it is the same recording.')}</span>
            </Notice>
          ) : null}
          {media.kind === 'video' ? (
            <video ref={player as RefObject<HTMLVideoElement>} className="bk-media-element" controls preload="metadata" src={url} aria-label={label} data-testid="media-element" onTimeUpdate={moved} onSeeked={moved} onLoadedMetadata={(e) => setLength(e.currentTarget.duration)} />
          ) : (
            <audio ref={player as RefObject<HTMLAudioElement>} className="bk-media-element" controls preload="metadata" src={url} aria-label={label} data-testid="media-element" onTimeUpdate={moved} onSeeked={moved} onLoadedMetadata={(e) => setLength(e.currentTarget.duration)} />
          )}
          <Waveform sourceId={data.id} url={url} sizeBytes={media.size_bytes} version={media.created_at} fileName={media.file_name} durationMs={knownMs} nowMs={nowMs} seek={(ms) => moveTo(ms, false)} />
          {media.found && media.stands_alone ? (
            <ClipTools sourceId={data.id} nowMs={nowMs} lengthMs={knownMs} readOnly={readOnly} seek={(ms) => moveTo(ms, false)} playSpan={playSpan} standsAlone media={{ url, sizeBytes: media.size_bytes, documentName: data.name }} />
          ) : media.found ? (
            <details className="bk-media-more clip-details" data-testid="clip-details">
              <summary>{t('Code a stretch of the recording')}</summary>
              <ClipTools sourceId={data.id} nowMs={nowMs} lengthMs={knownMs} readOnly={readOnly} seek={(ms) => moveTo(ms, false)} playSpan={playSpan} standsAlone={false} media={{ url, sizeBytes: media.size_bytes, documentName: data.name }} />
            </details>
          ) : null}
          {timed.length === 0 ? (
            media.stands_alone ? null : <p className="muted">{t('The turns of this document have no times, so the text cannot follow the recording.')}</p>
          ) : (
            <p className="bk-media-now" data-testid="media-now">
              {now ? (now.speaker ? t('Now: {speaker}: {turn}', { speaker: now.speaker, turn: turnLabel(now, wordsOf(now.start, now.end), 80).replace(/^\S+ /, '') }) : t('Now: {turn}', { turn: turnLabel(now, wordsOf(now.start, now.end), 80).replace(/^\S+ /, '') })) : t('Before the first turn.')}
            </p>
          )}
          <details className="bk-media-more" data-testid="media-more">
            <summary>{timed.length > 0 ? t('Move to a turn, play from the caret, follow the text, or unlink') : t('Unlink this recording')}</summary>
            {media.stands_alone ? <p className="muted">{t('Unlinking does not touch the recording. The stretches already coded stay in the project, but they cannot be played until a recording is linked again.')}</p> : null}
            {timed.length > 0 ? (
              <div className="bk-media-tools">
                <div className="field">
                  <label htmlFor="media-turn">{t('Move the recording to a turn')}</label>
                  <select
                    id="media-turn"
                    value=""
                    onChange={(e) => {
                      const u = turns.find((x) => String(x.id) === e.target.value);
                      if (u) moveTo(u.time_start_ms, false);
                    }}
                  >
                    <option value="">{t('Choose a turn…')}</option>
                    {turns.map((u) => (
                      <option key={u.id} value={u.id}>
                        {turnLabel(u, wordsOf(u.start, u.end))}
                      </option>
                    ))}
                  </select>
                </div>
                <Button
                  onPress={() => {
                    const sel = appStore.get().selection;
                    const ms = timeForOffset(timed, sel?.start ?? 0);
                    if (ms !== null) moveTo(ms, true);
                  }}
                  data-testid="media-play-caret"
                >{t('Play from the caret')}</Button>
                <Button
                  onPress={() => now && appStore.set({ pendingJump: { sourceId: data.id, start: now.start, end: now.end } })}
                  isDisabled={now === null}
                  data-testid="media-go-words"
                >{t('Go to the words playing now')}</Button>
                <Checkbox isSelected={follow} onChange={setFollow} className="check" data-testid="media-follow">
                  <div className="box" aria-hidden="true">
                    <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                      <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                    </svg>
                  </div>{t('Scroll the text to follow the recording')}</Checkbox>
              </div>
            ) : null}
            <div className="actions">{unlinkButton}</div>
          </details>
        </>
      )}
      {error ? <Notice>{error}</Notice> : null}
    </section>
  );
}

function LinkForm(props: { path: string; setPath: (v: string) => void; onLink: (e?: FormEvent) => void; onChoose: () => void; disabled: boolean }) {
  return (
    <form className="bk-media-link" onSubmit={props.onLink} noValidate>
      <div className="field">
        <label htmlFor="media-path">{t('Full path of the recording')}</label>
        <input id="media-path" type="text" value={props.path} onChange={(e) => props.setPath(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t('C:\\Recordings\\interview-1.mp3')} />
        <span className="help">{t('PrismQDA keeps where the recording is on this computer. It does not copy the recording into the project, and it does not turn speech into text.')}</span>
      </div>
      <div className="actions">
        <Button type="submit" className="primary" isDisabled={props.disabled} data-testid="media-link">{t('Link this recording')}</Button>
        {nativeDialogsAvailable ? (
          <Button onPress={props.onChoose} isDisabled={props.disabled}>{t('Choose a recording…')}</Button>
        ) : null}
      </div>
    </form>
  );
}
