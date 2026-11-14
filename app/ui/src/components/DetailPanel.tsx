import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, Checkbox, Label, TextArea, TextField } from 'react-aria-components';
import { call } from '../bridge';
import { formatWhen } from '../lib/eventWording';
import { lineLabel } from '../lib/lines';
import { describeHistory, initialsOf, memoNumber, summarizeHistory } from '../lib/provenance';
import { describeShape, sameShape } from '../lib/regions';
import { pathLabel } from '../lib/tree';
import { sliceScalar } from '../offsets';
import { changeCodingCode, commandClip, goTo, removeCoding, saveRationale, selectCode, selectCoding, setAnchor, setCodingRating, setPane } from '../state/actions';
import { appStore, focusApplyBox, lineRangeOf, useApp } from '../state/app';
import type { CodingHistoryEvent, CodingRow, MemoRow } from '../types';
import { Icon } from './Icon';
import { PassageLinksSection } from './PassageLinks';
import { SegmentQuote } from './SegmentQuote';
import { plural } from './common';
import { compareText, num, t, tn } from '../i18n';

/**
 * Segment Details: the coding that is selected, from the quote to where it came from. Reached from the reader, from a
 * stripe, and from a row of Retrieved Segments, and always describing the same coding. The close button hides the area;
 * the View tab of the ribbon brings it back.
 */
export function DetailPanel() {
  const data = useApp((s) => s.sourceData);
  const id = useApp((s) => s.selectedCodingId);
  const coding = data && id !== null ? (data.codings.find((c) => c.id === id) ?? null) : null;
  return (
    <section id="pane-details" className="pane-part details bk-card" aria-labelledby="h-details" tabIndex={-1}>
      <div className="bk-pane-head">
        <Icon name="ws-synth" size={20} className="bk-pane-icon" />
        <h2 id="h-details">{t('Segment Details')}</h2>
        <Button
          className="icon-button bk-close"
          aria-label={t('Hide Segment Details')}
          onPress={() => {
            setPane('details', false);
            // Back to the segment (the reader holds its place), so the keyboard is not left on a button that is gone.
            window.setTimeout(() => document.getElementById('main-content')?.focus(), 0);
          }}
        >
          <Icon name="ui-close" size={18} />
        </Button>
      </div>
      <div className="bk-details-body">
        {coding ? (
          <SegmentForm key={coding.id} coding={coding} />
        ) : (
          <p className="help" data-testid="coding-detail-empty">{t('No segment is selected. Choose a coded passage in the reader, or a row of Retrieved Segments, to see it here.')}</p>
        )}
      </div>
    </section>
  );
}

function SegmentForm({ coding }: { coding: CodingRow }) {
  const data = useApp((s) => s.sourceData)!;
  const codes = useApp((s) => s.codes);
  const coders = useApp((s) => s.coders);
  const anchors = useApp((s) => s.anchors);
  const cases = useApp((s) => s.cases);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const sourceName = useApp((s) => s.sources.find((x) => x.id === coding.source_id)?.name ?? data.name);
  const area = coding.region ?? null;
  // A stretch of a recording is an area in the lists but is told by its times, and can be played.
  const stretch = area !== null && area.shape === 'span' ? area : null;
  // An area of a page of a PDF is told with its page.
  const onPage = useApp((s) => area !== null && !!s.sources.find((x) => x.id === coding.source_id)?.pdf);
  const lineNumbers = useApp((s) => s.settings.lineNumbers);
  const linesStamp = useApp((s) => s.linesStamp);
  const [note, setNote] = useState(coding.rationale);
  const [options, setOptions] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [memos, setMemos] = useState<MemoRow[] | null>(null);
  const [history, setHistory] = useState<CodingHistoryEvent[] | null>(null);
  const code = codes.find((c) => c.id === coding.code_id);
  const quote = area ? '' : sliceScalar(data.text, data.index, coding.start, coding.end);
  const coder = coders.find((c) => c.id === coding.coder_id);
  const choices = useMemo(
    () =>
      codes
        .filter((c) => c.status === 'active' || c.id === coding.code_id)
        .map((c) => ({ id: c.id, label: pathLabel(codes, c.id) + (c.status !== 'active' ? ` (${c.status})` : '') }))
        .sort((a, b) => compareText(a.label, b.label)),
    [codes, coding.code_id],
  );
  const unit = coding.unit_id !== null ? data.units.findIndex((u) => u.id === coding.unit_id) : -1;
  // Every coding on exactly these words: the codes applied to this segment.
  const applied = useMemo(
    () => data.codings.filter((c) => (area ? !!c.region && (c.page ?? 1) === (coding.page ?? 1) && sameShape(c.region, area) : !c.region && c.start === coding.start && c.end === coding.end)),
    [data.codings, coding.start, coding.end, area],
  );
  // The lines are the ones drawn on screen, measured by the document; they are looked up again when it measures again.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const lines = useMemo(() => (lineNumbers && !area ? lineRangeOf(coding.start, coding.end) : null), [lineNumbers, coding.start, coding.end, linesStamp, data.index, area]);
  const caseNames = cases.filter((c) => c.source_ids?.includes(coding.source_id)).map((c) => c.name);

  useEffect(() => {
    let live = true;
    setMemos(null);
    call('memo.list', { scope: 'coding', target_id: coding.id })
      .then((m) => live && setMemos(m))
      .catch(() => live && setMemos([]));
    return () => {
      live = false;
    };
  }, [coding.id]);

  // The audit trail for this coding, read again after anything that adds to it.
  useEffect(() => {
    let live = true;
    call('coding.history', { id: coding.id })
      .then((h) => live && setHistory(h))
      .catch(() => live && setHistory(null));
    return () => {
      live = false;
    };
  }, [coding.id, coding.code_id, coding.rationale, coding.rating]);

  const summary = useMemo(() => (history ? summarizeHistory(history) : null), [history]);
  const who = coder ? coder.name : 'someone';
  const role = coder ? (coder.role === 'model' ? t('AI model') : t('Researcher')) : '';
  const origin = coding.origin === 'ai_applied' ? t('Applied by AI') : coding.origin === 'ai_suggestion_accepted' ? t('AI suggestion accepted') : null;
  const codedOn = summary?.firstAt ?? coding.created_at ?? null;

  return (
    <div className="coding-detail" data-testid="coding-detail">
      <h3 className="bk-h3" id="seg-selected">{t('Selected Segment')}</h3>
      {area ? (
        <div data-testid="coding-quote" aria-labelledby="seg-selected">
          <SegmentQuote text={describeShape(area, onPage ? 'page' : 'picture')} sourceId={coding.source_id} region={area} page={onPage ? (coding.page ?? 1) : null} color={code?.color ?? 'var(--line-strong)'} />
          {stretch ? (
            <Button className="img-tool" onPress={() => commandClip(coding.source_id, 'play', stretch.start_ms, stretch.end_ms)} data-testid="detail-play-clip">{t('Play this stretch')}</Button>
          ) : null}
        </div>
      ) : (
        <blockquote className="bk-quote" data-testid="coding-quote" aria-labelledby="seg-selected" style={{ ['--seg-color' as string]: code?.color ?? 'var(--line-strong)' }}>
          {`“${quote}”`}
        </blockquote>
      )}

      <dl className="bk-facts" data-testid="coding-facts">
        <div>
          <dt>{t('Source')}</dt>
          <dd>
            <Icon name="src-transcript" size={16} />
            {sourceName}
          </dd>
        </div>
        {lines ? (
          <div>
            <dt>{t('Lines')}</dt>
            <dd data-testid="coding-lines">
              {lineLabel(lines.first)}
              {lines.last !== lines.first ? ` – ${lineLabel(lines.last)}` : ''} ({tn(lines.last - lines.first + 1, { one: '{count} line', other: '{count} lines' })})
            </dd>
          </div>
        ) : null}
        <div>
          <dt>{t('Case')}</dt>
          <dd data-testid="coding-case">
            {caseNames.length > 0 ? (
              <>
                <Icon name="src-case" size={16} />
                {caseNames.join(', ')}
              </>
            ) : (
              <span className="muted">{t('None')}</span>
            )}
          </dd>
        </div>
        <div className="visually-hidden">
          <dt>{t('Where')}</dt>
          <dd>
            {stretch ? t('A stretch of the recording') : area ? t('An area of the picture') : t('Characters {start} to {end}', { start: num(coding.start), end: num(coding.end) })}
            {unit >= 0 ? t(', unit {unit}', { unit: unit + 1 }) : ''}
          </dd>
        </div>
      </dl>

      <section className="bk-section" aria-labelledby="seg-codes">
        <div className="bk-section-head">
          <h3 id="seg-codes" className="bk-h3">{t('Applied Codes ({count})', { count: applied.length })}</h3>
          {readOnly ? null : (
            <Button className="icon-button" aria-label={stretch ? t('Add another code to this stretch') : area ? t('Add another code to this area') : t('Add another code to these words')} onPress={() => focusApplyBox()}>
              <Icon name="ui-plus" size={18} />
            </Button>
          )}
        </div>
        <div className="bk-applied">
          <ul className="chip-list" data-testid="applied-codes">
            {applied.map((c) => {
              const k = codes.find((x) => x.id === c.code_id);
              return (
                <li key={c.id} className={`code-chip${c.id === coding.id ? ' current' : ''}`}>
                  <Button className="chip-name" onPress={() => selectCoding(c.id)} aria-label={c.id === coding.id ? t('{name}, the one shown', { name: k?.name ?? t('A code') }) : t('{name}: show this coding', { name: k?.name ?? t('A code') })}>
                    <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: k?.color ?? '#808080' }} />
                    {k?.name ?? t('A code')}
                  </Button>
                  {readOnly ? null : (
                    <Button className="chip-remove" aria-label={stretch ? t('Remove the code {name} from this stretch', { name: k?.name ?? '' }) : area ? t('Remove the code {name} from this area', { name: k?.name ?? '' }) : t('Remove the code {name} from these words', { name: k?.name ?? '' })} onPress={() => void removeCoding(c.id)}>
                      <Icon name="ui-close" size={12} />
                    </Button>
                  )}
                </li>
              );
            })}
          </ul>
          <Button className="icon-button bk-disclose" aria-expanded={options} aria-controls="seg-options" aria-label={t('Options for this coding')} onPress={() => setOptions((o) => !o)}>
            <Icon name="ui-chevron-down" size={16} className={options ? 'open' : ''} />
          </Button>
        </div>
        {options ? (
          <div id="seg-options" className="bk-options">
            <div className="field">
              <label htmlFor="coding-code">{t('Code')}</label>
              <select id="coding-code" value={coding.code_id} disabled={readOnly} onChange={(e) => void changeCodingCode(coding.id, Number(e.target.value))}>
                {choices.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.label}
                  </option>
                ))}
              </select>
            </div>
            <TextField value={note} onChange={setNote} className="field" isReadOnly={readOnly}>
              <Label>{t('Note: why did you code it this way?')}</Label>
              <TextArea rows={3} />
            </TextField>
            {readOnly ? null : (
              <div className="actions">
                <Button onPress={() => void saveRationale(coding.id, note)} isDisabled={note === coding.rationale}>{t('Save note')}</Button>
              </div>
            )}
            {area ? null : (
              <Checkbox isSelected={anchors.has(coding.id)} onChange={(v) => void setAnchor(coding.id, v)} className="check" isDisabled={readOnly}>
                <div className="box" aria-hidden="true">
                  <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
                    <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
                  </svg>
                </div>{t('Use as an example of {name}', { name: code?.name ?? t('this code') })}
              </Checkbox>
            )}
            {readOnly ? null : (
              <div className="actions">
                {confirming ? (
                  <>
                    <Button className="danger" onPress={() => void removeCoding(coding.id)}>{t('Yes, remove this coding')}</Button>
                    <Button onPress={() => setConfirming(false)}>{t('Keep it')}</Button>
                  </>
                ) : (
                  <Button onPress={() => setConfirming(true)}>{t('Remove this coding…')}</Button>
                )}
              </div>
            )}
          </div>
        ) : null}
      </section>

      <section className="bk-section" aria-labelledby="seg-memo">
        <div className="bk-section-head">
          <h3 id="seg-memo" className="bk-h3">{memos ? t('Memo ({count})', { count: memos.length }) : t('Memo')}
          </h3>
          <Button
            className="icon-button"
            aria-label={memos && memos.length > 0 ? t('Open the memos about this coding') : t('Write a memo about this coding')}
            onPress={() => appStore.set({ workspace: 'synthesize' })}
          >
            <Icon name={memos && memos.length > 0 ? 'ui-external' : 'ui-plus'} size={18} />
          </Button>
        </div>
        {memos && memos.length > 0 ? (
          <ul className="memo-links">
            {memos.map((m) => (
              <li key={m.id}>
                <Icon name="memo-general" size={16} />
                <span>
                  {memoNumber(m.id)}: {m.title.trim() || t('Untitled memo')}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">{memos === null ? t('Looking for memos…') : t('No memo about this coding yet.')}</p>
        )}
      </section>

      <PassageLinksSection codingId={coding.id} />

      <section className="bk-section" aria-labelledby="seg-rating">
        <div className="bk-section-head">
          <h3 id="seg-rating" className="bk-h3">{t('Weight / Rating')}</h3>
          <span className="bk-info" title={t("An analytic rating of this coding on a scale of 1 to 5 stars. It is yours to use as you see fit, and it is separate from any statistical weight or an AI's confidence.")}>
            <Icon name="st-info" size={16} />
            <span className="visually-hidden">{t("An analytic rating of this coding on a scale of 1 to 5 stars. It is separate from any statistical weight or an AI's confidence.")}</span>
          </span>
        </div>
        {code?.rated ? (
          <>
            {code.rating_label ? (
              <p className="muted bk-rating-name" data-testid="rating-label">
                {code.rating_label}
              </p>
            ) : null}
            <Rating value={coding.rating ?? null} disabled={readOnly} onChange={(r) => void setCodingRating(coding.id, r)} />
            {code.rating_low || code.rating_high ? (
              <p className="muted bk-scale-ends" data-testid="rating-scale">
                {[code.rating_low ? `1 = ${code.rating_low}` : '', code.rating_high ? `5 = ${code.rating_high}` : ''].filter(Boolean).join('  ·  ')}
              </p>
            ) : null}
          </>
        ) : (
          <p className="muted" data-testid="rating-off">
            {code ? t('{name} is not a rated code, so its codings are not rated. ', { name: code.name }) : t('This coding is not rated. ')}
            {code && !readOnly ? (
              <Button
                className="bk-link"
                onPress={() => {
                  selectCode(code.id, { openDetails: false });
                  goTo('codebook');
                }}
              >{t('Turn on Weighted / Rated in the Codebook Editor')}</Button>
            ) : null}
          </p>
        )}
      </section>

      <details className="bk-section provenance" open data-testid="provenance">
        <summary>
          <h3 className="bk-h3">{t('Provenance')}</h3>
          <Icon name="ui-chevron-down" size={16} className="bk-summary-chevron" />
        </summary>
        <dl className="bk-facts bk-provenance">
          <div>
            <dt>{t('Coded by:')}</dt>
            <dd>
              <span className="bk-avatar small" aria-hidden="true">
                {initialsOf(who)}
              </span>
              {who}
              {role ? ` (${role})` : ''}
            </dd>
          </div>
          {origin ? (
            <div>
              <dt>{t('How:')}</dt>
              <dd>{origin}</dd>
            </div>
          ) : null}
          {codedOn ? (
            <div>
              <dt>{t('Coded on:')}</dt>
              <dd>{formatWhen(codedOn)}</dd>
            </div>
          ) : null}
          {summary?.lastAt ? (
            <div>
              <dt>{t('Last modified:')}</dt>
              <dd>{formatWhen(summary.lastAt)}</dd>
            </div>
          ) : null}
          {summary ? (
            <div>
              <dt>{t('History:')}</dt>
              <dd>{describeHistory(summary)}</dd>
            </div>
          ) : null}
        </dl>
      </details>
    </div>
  );
}

/**
 * The rating: five stars, a radio group. The arrow keys move between 1 and 5, and "Not rated" is its own choice, so an
 * unrated coding is never mistaken for a low one. The number is written out beside the stars.
 */
function Rating(props: { value: number | null; disabled: boolean; onChange: (value: number | null) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const move = (to: number) => {
    const next = Math.min(5, Math.max(1, to));
    props.onChange(next);
    window.setTimeout(() => refs.current[next - 1]?.focus(), 0);
  };
  return (
    <div className="bk-rating">
      <div role="radiogroup" aria-labelledby="seg-rating" className="bk-stars" data-testid="rating">
        {[1, 2, 3, 4, 5].map((n) => {
          const on = props.value !== null && n <= props.value;
          const current = props.value === n || (props.value === null && n === 1);
          return (
            <button
              key={n}
              ref={(el) => {
                refs.current[n - 1] = el;
              }}
              type="button"
              role="radio"
              aria-checked={props.value === n}
              aria-label={tn(n, { one: '{count} star out of 5', other: '{count} stars out of 5' })}
              tabIndex={current ? 0 : -1}
              disabled={props.disabled}
              className={`bk-star${on ? ' on' : ''}`}
              onClick={() => props.onChange(n)}
              onKeyDown={(e) => {
                const v = props.value ?? 0;
                if (e.key === 'ArrowRight' || e.key === 'ArrowUp') {
                  e.preventDefault();
                  move(v + 1);
                } else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') {
                  e.preventDefault();
                  move(v - 1);
                } else if (e.key === 'Home') {
                  e.preventDefault();
                  move(1);
                } else if (e.key === 'End') {
                  e.preventDefault();
                  move(5);
                } else if (e.key === 'Delete' || e.key === 'Backspace') {
                  e.preventDefault();
                  props.onChange(null);
                }
              }}
            >
              <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" focusable="false">
                <path d="M12 3.2l2.6 5.6 6.1.7-4.5 4.2 1.2 6-5.4-3-5.4 3 1.2-6-4.5-4.2 6.1-.7z" />
              </svg>
            </button>
          );
        })}
      </div>
      <span className="bk-rating-value" data-testid="rating-value">
        {props.value === null ? t('Not rated') : `(${props.value} / 5)`}
      </span>
      {props.value !== null && !props.disabled ? (
        <Button className="bk-link" aria-label={t('Clear the rating')} onPress={() => props.onChange(null)}>{t('Clear')}</Button>
      ) : null}
    </div>
  );
}
