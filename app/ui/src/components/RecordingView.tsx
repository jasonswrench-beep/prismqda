// The body of a document that stands for a recording imported without captions: the player with the tools for coding stretches, and the
// description (the words that tell what the recording holds, which is all the text it has). The player is here and not above the body, as
// it is for a transcript, so that the whole of it scrolls together: the tools for coding stretches are tall.

import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { saveRecordingDescription } from '../state/actions';
import { useApp, type SourceData } from '../state/app';
import { MediaPlayer } from './MediaPlayer';
import { SelectionLine } from './SelectionLine';
import { t, tn } from '../i18n';

export function RecordingView(props: { data: SourceData }) {
  const { data } = props;
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const stretches = data.codings.filter((c) => c.region?.shape === 'span').length;
  const [editing, setEditing] = useState(false);
  const [words, setWords] = useState(data.text);
  useEffect(() => {
    setWords(data.text);
    setEditing(false);
  }, [data.id, data.text]);

  return (
    <div className="rec-view" data-testid="recording-view">
      <MediaPlayer data={data} isTranscript />
      <section className="bk-section" aria-labelledby="rec-describe-h">
        <div className="img-describe">
          <h3 className="bk-h3" id="rec-describe-h">{t('Description')}</h3>
          {editing ? (
            <div className="field">
              <label htmlFor="rec-description">{t('What the recording holds: who speaks, where and when it was made, and what it is about')}</label>
              <textarea id="rec-description" rows={4} value={words} onChange={(e) => setWords(e.target.value)} data-testid="recording-description-box" />
              <div className="actions">
                <Button
                  className="primary"
                  onPress={() => {
                    void saveRecordingDescription(data.id, words).then((ok) => ok && setEditing(false));
                  }}
                  data-testid="recording-description-save"
                >{t('Save description')}</Button>
                <Button
                  onPress={() => {
                    setWords(data.text);
                    setEditing(false);
                  }}
                >{t('Cancel')}</Button>
              </div>
            </div>
          ) : (
            <>
              <p data-testid="recording-description">{data.text}</p>
              {readOnly ? null : (
                <Button onPress={() => setEditing(true)} data-testid="recording-describe">{t('Edit the description')}</Button>
              )}
            </>
          )}
        </div>
      </section>
      <SelectionLine />
      <p className="muted" data-testid="recording-summary">
        {stretches === 0
          ? t('This recording has no text of its own, because PrismQDA does not turn speech into text. Code it by stretches of time with the tools above, or import a transcript and link this recording to it.')
          : tn(stretches, { one: '{count} stretch of this recording is coded.', other: '{count} stretches of this recording are coded.' })}
      </p>
    </div>
  );
}
