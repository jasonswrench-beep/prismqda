// Playing the clips of a code one after another: every coded stretch of a recording that is in the list of retrieved segments, in the order
// the list is in, across recordings. Nothing plays until a button is pressed, and one element plays them all (it is given the address of
// the next recording only when the clip changes recordings).

import { useEffect, useRef, useState } from 'react';
import { Button } from 'react-aria-components';
import { mediaUrl } from '../bridge';
import { spanShort } from '../lib/clips';
import { t } from '../i18n';

export interface PlaylistClip {
  codingId: number;
  sourceId: number;
  sourceName: string;
  codeName: string;
  startMs: number;
  endMs: number;
  kind: 'audio' | 'video';
}

/** The words that say which clip is playing and where it is in the list. */
export function clipLine(clip: PlaylistClip, index: number, count: number): string {
  return t('Clip {n} of {count}: {document}, {span}, {code}.', { n: String(index + 1), count: String(count), document: clip.sourceName, span: spanShort(clip.startMs, clip.endMs), code: clip.codeName });
}

export function ClipPlaylist(props: { clips: PlaylistClip[] }) {
  const { clips } = props;
  const video = useRef<HTMLVideoElement>(null);
  const loaded = useRef<string>('');
  const [at, setAt] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const listRef = useRef(clips);
  listRef.current = clips;
  const atRef = useRef<number | null>(null);
  atRef.current = at;

  /** Starts clip `i` of the list: the recording is loaded first when it is another one than the clip before's. */
  const start = (i: number) => {
    const list = listRef.current;
    const el = video.current;
    if (!el || i < 0 || i >= list.length) return;
    const clip = list[i];
    atRef.current = i;
    setAt(i);
    setPaused(false);
    setNote(null);
    const url = mediaUrl(clip.sourceId);
    const begin = () => {
      el.currentTime = clip.startMs / 1000;
      void el.play().catch(() => {
        setNote(t('{document} could not be played. Is its recording where it was? Going on to the next clip.', { document: clip.sourceName }));
        next(i);
      });
    };
    if (loaded.current !== url) {
      loaded.current = url;
      el.addEventListener('loadedmetadata', begin, { once: true });
      el.src = url;
      el.load();
    } else {
      begin();
    }
  };

  const stop = () => {
    video.current?.pause();
    atRef.current = null;
    setAt(null);
    setPaused(false);
  };

  const next = (from: number) => {
    if (from + 1 < listRef.current.length) start(from + 1);
    else {
      video.current?.pause();
      atRef.current = null;
      setAt(null);
      setPaused(false);
      setNote(t('That was the last clip.'));
    }
  };

  // Each clip is stopped where it ends and the next one begins. The element reports its time only a few times a second, which would let a
  // clip run on past its end, so the time is looked at often while a clip plays.
  const onTime = () => {
    const i = atRef.current;
    const el = video.current;
    if (i === null || !el || el.paused) return;
    if (el.currentTime * 1000 >= listRef.current[i].endMs) next(i);
  };
  useEffect(() => {
    if (at === null) return;
    const timer = window.setInterval(onTime, 40);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [at]);

  // A clip whose end is beyond the end of its recording ends with the recording.
  const onEnded = () => {
    const i = atRef.current;
    if (i !== null) next(i);
  };

  // A recording that cannot be loaded is told and passed over.
  const onError = () => {
    const i = atRef.current;
    if (i === null) return;
    loaded.current = '';
    setNote(t('{document} could not be played. Is its recording where it was? Going on to the next clip.', { document: listRef.current[i].sourceName }));
    next(i);
  };

  // Leaving the page stops the sound.
  useEffect(() => {
    const el = video.current;
    return () => el?.pause();
  }, []);

  // The list changed under it (a filter): what is playing goes on, but the position is kept inside the new list.
  useEffect(() => {
    if (at !== null && at >= clips.length) stop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clips.length]);

  const documents = new Set(clips.map((c) => c.sourceId)).size;
  const current = at !== null ? clips[at] : null;
  return (
    <div className="bk-playlist" role="group" aria-label={t('Play the clips')} data-testid="clip-playlist">
      <p className="help">
        {t('{count} coded stretches of recordings from {documents} are in this list. Play them one after another, in the order of the list.', { count: String(clips.length), documents: documents === 1 ? t('1 document') : t('{n} documents', { n: String(documents) }) })}
      </p>
      <div className="actions">
        <Button className="primary" onPress={() => start(0)} isDisabled={clips.length === 0} data-testid="playlist-play">{t('Play all the clips')}</Button>
        <Button onPress={() => current && start(Math.max(0, at! - 1))} isDisabled={current === null || at === 0} data-testid="playlist-previous">{t('Previous clip')}</Button>
        <Button
          onPress={() => {
            const el = video.current;
            if (!el || current === null) return;
            if (paused) {
              void el.play();
              setPaused(false);
            } else {
              el.pause();
              setPaused(true);
            }
          }}
          isDisabled={current === null}
          data-testid="playlist-pause"
        >{paused ? t('Resume') : t('Pause')}</Button>
        <Button onPress={() => current && next(at!)} isDisabled={current === null} data-testid="playlist-next">{t('Next clip')}</Button>
        <Button onPress={stop} isDisabled={current === null} data-testid="playlist-stop">{t('Stop')}</Button>
      </div>
      <p className="bk-playlist-now" role="status" aria-live="polite" data-testid="playlist-now">
        {current ? `${paused ? t('Paused. ') : ''}${clipLine(current, at!, clips.length)}` : note ?? t('Nothing is playing.')}
      </p>
      <video ref={video} className="bk-playlist-video" hidden={current === null || current.kind !== 'video'} preload="metadata" aria-label={t('The recording being played')} onEnded={onEnded} onError={onError} data-testid="playlist-element" />
    </div>
  );
}
