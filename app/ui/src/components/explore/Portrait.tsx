// Document portrait and codeline (decision 164): where the coded passages fall along each document. A mark is a button that opens the passage
// in its document; its name says the code, the words, and where it is, and a table gives the same numbers, so nothing is told by color alone.

import { useEffect, useMemo, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { buildPortraits, share, type Portrait } from '../../lib/portrait';
import { openAt } from '../../state/actions';
import { useApp } from '../../state/app';
import type { SegmentRow } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { compareText, num, t, tn } from '../../i18n';

const LANE = 12;

/** The strip of one document: every mark at its place, stacked where passages overlap. */
function Strip(props: { portrait: Portrait; onlyCode?: number }) {
  const p = props.portrait;
  const single = props.onlyCode !== undefined;
  const marks = single ? p.marks.filter((m) => m.codeId === props.onlyCode) : p.marks;
  const lanes = single ? 1 : p.lanes;
  return (
    <div
      className="pt-strip"
      style={{ height: lanes * LANE + 4 }}
      role="group"
      aria-label={t('Where the codes fall in {name}', { name: p.name })}
      data-testid="pt-strip"
      onKeyDown={(e) => {
        // One tab stop for the strip; the arrow keys move between its marks, in the order of the text.
        if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) return;
        const all = [...e.currentTarget.querySelectorAll<HTMLButtonElement>('button.pt-mark')];
        const at = all.indexOf(document.activeElement as HTMLButtonElement);
        if (at < 0) return;
        e.preventDefault();
        const next = e.key === 'Home' ? 0 : e.key === 'End' ? all.length - 1 : Math.min(all.length - 1, Math.max(0, at + (e.key === 'ArrowRight' ? 1 : -1)));
        all[next]?.focus();
      }}
    >
      {marks.map((m, i) => {
        const left = share(m.start, p.characters);
        const width = Math.max(share(m.end, p.characters) - left, 0.5);
        return (
          <button
            key={m.codingId}
            type="button"
            className="pt-mark"
            tabIndex={i === 0 ? 0 : -1}
            style={{ left: `${left}%`, width: `${width}%`, top: (single ? 0 : m.lane * LANE) + 2, background: m.color }}
            aria-label={t('{code}: “{words}”, from {from} to {to} percent of {name}', { code: m.codeName, words: m.text.replace(/\s+/g, ' ').slice(0, 80), from: Math.round(left), to: Math.round(share(m.end, p.characters)), name: p.name })}
            title={`${m.codeName}: ${m.text.replace(/\s+/g, ' ').slice(0, 80)}`}
            onClick={() => void openAt(p.sourceId, m.start, m.end, m.codingId)}
            data-testid="pt-mark"
          />
        );
      })}
    </div>
  );
}

/** The same numbers as the picture, as a table. */
function UseTable(props: { portrait: Portrait }) {
  const p = props.portrait;
  return (
    <details className="pt-table">
      <summary>{t('The numbers for {name}', { name: p.name })}</summary>
      <div className="table-wrap" tabIndex={-1}>
        <table className="data" data-testid="pt-table">
          <caption>{t('What each code does in {name}', { name: p.name })}</caption>
          <thead>
            <tr>
              <th scope="col">{t('Code')}</th>
              <th scope="col">{t('Passages')}</th>
              <th scope="col">{t('Characters coded')}</th>
              <th scope="col">{t('Share of the document')}</th>
              <th scope="col">{t('First passage starts at')}</th>
              <th scope="col">{t('Last passage ends at')}</th>
            </tr>
          </thead>
          <tbody>
            {p.uses.map((u) => (
              <tr key={u.codeId}>
                <th scope="row">{u.name}</th>
                <td>{num(u.passages)}</td>
                <td>{num(u.characters)}</td>
                <td>{`${num(Math.round((u.characters / p.characters) * 1000) / 10)}%`}</td>
                <td>{`${num(Math.round(u.from))}%`}</td>
                <td>{`${num(Math.round(u.to))}%`}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  );
}

/** Explore > Document Portrait: the portrait of every document, or the codeline of one. */
export function DocumentPortrait() {
  const sources = useApp((s) => s.sources);
  const codes = useApp((s) => s.codes);
  const version = useApp((s) => s.dataVersion);
  const [segments, setSegments] = useState<SegmentRow[] | null>(null);
  const [cut, setCut] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<'portrait' | 'codeline'>('portrait');
  const [picked, setPicked] = useState<number | null>(null);
  const [leaveOut, setLeaveOut] = useState<ReadonlySet<number>>(new Set());

  useEffect(() => {
    let live = true;
    call('segments.list', { limit: 5000, workingSet: true })
      .then((r) => {
        if (!live) return;
        setSegments(r.segments);
        setCut(r.truncated);
        setError(null);
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version]);

  const portraits = useMemo(() => (segments ? buildPortraits(segments, sources, codes, leaveOut) : []), [segments, sources, codes, leaveOut]);
  const usedCodes = useMemo(() => {
    if (!segments) return [];
    const ids = new Set(segments.filter((s) => s.kind === 'text').map((s) => s.code_id));
    return codes.filter((c) => ids.has(c.id)).sort((a, b) => compareText(a.name, b.name));
  }, [segments, codes]);
  const one = portraits.find((p) => p.sourceId === picked) ?? portraits[0] ?? null;

  return (
    <div data-testid="portrait-tool">
      <p className="help">{t('Where the coded passages fall along each document, from its first word to its last. Every mark is a passage: press it to open the passage in its document. Only words are drawn; areas of pictures and stretches of recordings have no place along a text.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      <div className="field">
        <label htmlFor="pt-view">{t('Draw')}</label>
        <select id="pt-view" value={view} onChange={(e) => setView(e.target.value as 'portrait' | 'codeline')} data-testid="pt-view">
          <option value="portrait">{t('A portrait of every document')}</option>
          <option value="codeline">{t('The codeline of one document')}</option>
        </select>
      </div>
      {view === 'codeline' && portraits.length > 0 ? (
        <div className="field">
          <label htmlFor="pt-doc">{t('Document')}</label>
          <select id="pt-doc" value={one?.sourceId ?? ''} onChange={(e) => setPicked(Number(e.target.value))} data-testid="pt-doc">
            {portraits.map((p) => (
              <option key={p.sourceId} value={p.sourceId}>{p.name}</option>
            ))}
          </select>
        </div>
      ) : null}
      {usedCodes.length > 1 ? (
        <details className="pt-codes">
          <summary>{leaveOut.size === 0 ? t('Codes drawn: all {count}', { count: usedCodes.length }) : t('Codes drawn: {n} of {count}', { n: usedCodes.length - leaveOut.size, count: usedCodes.length })}</summary>
          <ul className="pt-code-list">
            {usedCodes.map((c) => (
              <li key={c.id}>
                <Tick isSelected={!leaveOut.has(c.id)} onChange={(on) => setLeaveOut((prev) => { const next = new Set(prev); if (on) next.delete(c.id); else next.add(c.id); return next; })} testId="pt-code">
                  <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: c.color }} />
                  {c.name}
                </Tick>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      {cut ? <p className="muted" data-testid="pt-cut">{t('Only the first 5,000 coded passages are drawn; narrow the working set to see the rest.')}</p> : null}
      {segments === null && !error ? <p className="muted">{t('Looking for coded passages…')}</p> : null}
      {segments !== null && portraits.length === 0 ? <p className="help" data-testid="pt-empty">{t('No document has coded words to draw yet (with the codes and the working set as they are now).')}</p> : null}

      {view === 'portrait'
        ? portraits.map((p) => (
            <figure key={p.sourceId} className="pt-figure" data-testid="pt-portrait">
              <figcaption>
                <h3 className="bk-h3">{p.name}</h3>
                <p className="muted">{t('{coverage} percent of the document is coded, with {passages} and {codes}.', { coverage: Math.round(p.coverage), passages: tn(p.marks.length + p.notDrawn, { one: '{count} passage', other: '{count} passages' }), codes: tn(p.uses.length, { one: '{count} code', other: '{count} codes' }) })}</p>
              </figcaption>
              <Strip portrait={p} />
              <div className="pt-axis" aria-hidden="true">
                <span>0%</span>
                <span>50%</span>
                <span>100%</span>
              </div>
              {p.notDrawn > 0 ? <p className="muted">{t('{count} passages are not drawn, because only the first 600 are; the table has every passage.', { count: num(p.notDrawn) })}</p> : null}
              <ul className="pt-legend" aria-label={t('The codes in {name}', { name: p.name })}>
                {p.uses.map((u) => (
                  <li key={u.codeId}>
                    <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: u.color }} />
                    {`${u.name} (${num(u.passages)})`}
                  </li>
                ))}
              </ul>
              <UseTable portrait={p} />
            </figure>
          ))
        : one ? (
            <figure className="pt-figure" data-testid="pt-codeline">
              <figcaption>
                <h3 className="bk-h3">{one.name}</h3>
                <p className="muted">{t('One row for each code, along the document from its first word to its last.')}</p>
              </figcaption>
              <ul className="pt-codeline">
                {one.uses.map((u) => (
                  <li key={u.codeId} className="pt-row" data-testid="pt-codeline-row">
                    <span className="pt-row-label">
                      <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: u.color }} />
                      {`${u.name} (${num(u.passages)})`}
                    </span>
                    <Strip portrait={one} onlyCode={u.codeId} />
                  </li>
                ))}
              </ul>
              <div className="pt-axis pt-axis-line" aria-hidden="true">
                <span>0%</span>
                <span>50%</span>
                <span>100%</span>
              </div>
              <UseTable portrait={one} />
            </figure>
          ) : null}
      <p className="actions">
        <Button onPress={() => setView(view === 'portrait' ? 'codeline' : 'portrait')} data-testid="pt-switch">{view === 'portrait' ? t('See the codeline of one document') : t('See the portrait of every document')}</Button>
      </p>
    </div>
  );
}
