// A summary of each code: how often it was used, in how many documents and cases, by how many coders, and what it occurs with.

import { useEffect, useMemo, useState } from 'react';
import { Button } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { goTo, openAt, selectCode, setActiveCodes } from '../../state/actions';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { CodeProfile, CodeSummaryRow, CountedName, SegmentRow } from '../../types';
import { Notice, plural } from '../common';
import { SegmentQuote } from '../SegmentQuote';
import { codeChoices } from './shared';
import { num, t, tn } from '../../i18n';

export function CodeSummary() {
  const codes = useApp((s) => s.codes);
  const version = useApp((s) => s.dataVersion);
  const codings = useApp((s) => s.sources.reduce((n, x) => n + x.codings, 0));
  const selectedCode = useApp((s) => s.selectedCodeId);
  const [rows, setRows] = useState<CodeSummaryRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [chosen, setChosen] = useState<number | null>(selectedCode);

  useEffect(() => {
    let live = true;
    call('explore.summary', {})
      .then((r) => live && setRows(r))
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [version, codings, codes.length]);

  const ordered = useMemo(() => {
    const byId = new Map((rows ?? []).map((r) => [r.code_id, r]));
    return codeChoices(codes).flatMap((c) => {
      const r = byId.get(c.id);
      return r ? [{ row: r, depth: c.depth ?? 0, color: c.color ?? '#808080' }] : [];
    });
  }, [rows, codes]);

  return (
    <div className="ex-summary" data-testid="summary-tool">
      {error ? <Notice>{error}</Notice> : null}
      {rows === null ? (
        <p className="muted" role="status">{t('Counting…')}</p>
      ) : ordered.length === 0 ? (
        <p className="help">{t('There are no codes yet. New Code makes the first one.')}</p>
      ) : (
        <div className="ex-summary-grid">
          <div className="ex-table-wrap">
            <table className="ex-matrix ex-summary-table" data-testid="summary-table">
              <caption>{t('Each code, with the codings of its sub-codes counted in. Choose a code to see more.')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Code')}</th>
                  <th scope="col">{t('Codings')}</th>
                  <th scope="col">{t('Excerpts')}</th>
                  <th scope="col">{t('Documents')}</th>
                  <th scope="col">{t('Cases')}</th>
                  <th scope="col">{t('Coders')}</th>
                </tr>
              </thead>
              <tbody>
                {ordered.map(({ row, depth, color }) => (
                  <tr key={row.code_id} className={row.code_id === chosen ? 'current' : undefined}>
                    <th scope="row" style={{ paddingInlineStart: `${0.6 + depth * 1}rem` }}>
                      <Button
                        className="bk-link ex-summary-pick"
                        aria-pressed={row.code_id === chosen}
                        aria-label={row.code_id === chosen ? t('{name}, shown at the right', { name: row.name }) : t('{name}: show its summary', { name: row.name })}
                        onPress={() => {
                          setChosen(row.code_id);
                          announce(t('{name}: {codings} in {documents}.', { name: row.name, codings: tn(row.assignments, { one: '{count} coding', other: '{count} codings' }), documents: tn(row.documents, { one: '{count} document', other: '{count} documents' }) }));
                        }}
                      >
                        <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: color }} />
                        {row.name}
                      </Button>
                    </th>
                    <td className="ex-num">{num(row.assignments)}</td>
                    <td className="ex-num">{num(row.excerpts)}</td>
                    <td className="ex-num">{num(row.documents)}</td>
                    <td className="ex-num">{num(row.cases)}</td>
                    <td className="ex-num">{num(row.coders)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="bk-summary-detail">
            {chosen === null ? <p className="help">{t('Choose a code in the list to see where it was used, by whom, and what it occurs with.')}</p> : <Profile key={chosen} codeId={chosen} />}
          </div>
        </div>
      )}
    </div>
  );
}

function CountedTable({ title, rows, noun }: { title: string; rows: CountedName[]; noun: string }) {
  if (rows.length === 0) return null;
  const shown = rows.slice(0, 12);
  return (
    <div className="ex-table-wrap">
      <table className="ex-matrix ex-mini">
        <caption>{title}</caption>
        <thead>
          <tr>
            <th scope="col">{noun}</th>
            <th scope="col">{t('Codings')}</th>
            <th scope="col">{t('Excerpts')}</th>
          </tr>
        </thead>
        <tbody>
          {shown.map((r) => (
            <tr key={r.id}>
              <th scope="row">{r.name}</th>
              <td className="ex-num">{num(r.assignments)}</td>
              <td className="ex-num">{num(r.excerpts)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length > shown.length ? <p className="muted">{t('Showing {count} of {count2}.', { count: shown.length, count2: rows.length })}</p> : null}
    </div>
  );
}

function Profile({ codeId }: { codeId: number }) {
  const code = useApp((s) => s.codes.find((c) => c.id === codeId));
  const version = useApp((s) => s.dataVersion);
  const [profile, setProfile] = useState<CodeProfile | null>(null);
  const [samples, setSamples] = useState<SegmentRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setProfile(null);
    call('explore.profile', { code_id: codeId, include_descendants: true })
      .then(async (p) => {
        if (!live) return;
        setProfile(p);
        if (p.coding_ids.length > 0) {
          const s = await call('segments.list', { coding_ids: p.coding_ids.slice(0, 5), limit: 5 });
          if (live) setSamples(s.segments);
        } else {
          setSamples([]);
        }
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
  }, [codeId, version]);
  if (error) return <Notice>{error}</Notice>;
  if (!profile || !code) {
    return (
      <p className="muted" role="status">{t('Looking at {name}…', { name: code?.name ?? t('the code') })}</p>
    );
  }
  const s = profile.summary;
  return (
    <section aria-labelledby="h-profile" data-testid="profile">
      <h3 id="h-profile" className="bk-h3">
        {code.name}
      </h3>
      {code.definition.trim() !== '' ? <p className="ex-profile-definition">{code.definition}</p> : <p className="muted">{t('This code has no definition yet.')}</p>}
      <p data-testid="profile-counts">
        {t('{codings} ({distinct_excerpts}) in {documents} and {cases}, by {coders}.', { codings: tn(s.assignments, { one: '{count} coding', other: '{count} codings' }), distinct_excerpts: tn(s.excerpts, { one: '{count} distinct excerpt', other: '{count} distinct excerpts' }), documents: tn(s.documents, { one: '{count} document', other: '{count} documents' }), cases: tn(s.cases, { one: '{count} case', other: '{count} cases' }), coders: tn(s.coders, { one: '{count} coder', other: '{count} coders' }) })}
        {s.assignments !== s.own_assignments ? ' ' + t('{codings} are of the code itself; the rest are of its sub-codes.', { codings: tn(s.own_assignments, { one: '{count} coding', other: '{count} codings' }) }) : ''}
      </p>
      <div className="actions">
        <Button
          onPress={() => {
            setActiveCodes([codeId]);
            goTo('code');
          }}
        >{t('Show every passage in Retrieved Segments')}</Button>
        <Button
          onPress={() => {
            selectCode(codeId, { openDetails: false });
            goTo('codebook');
          }}
        >{t('Open in the Codebook Editor')}</Button>
      </div>
      <CountedTable title={t('Where it was used, by document')} rows={profile.by_document} noun={t('Document')} />
      <CountedTable title={t('By case')} rows={profile.by_case} noun={t('Case')} />
      <CountedTable title={t('By coder')} rows={profile.by_coder} noun={t('Coder')} />
      {profile.together_with.length > 0 ? (
        <div className="ex-table-wrap">
          <table className="ex-matrix ex-mini" data-testid="profile-together">
            <caption>{t('Codes whose words overlap this code’s (by the same coder)')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Code')}</th>
                <th scope="col">{t('Overlapping pairs')}</th>
              </tr>
            </thead>
            <tbody>
              {profile.together_with.map((r) => (
                <tr key={r.id}>
                  <th scope="row">{r.name}</th>
                  <td className="ex-num">{num(r.assignments)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="muted">{t('No other code overlaps this one.')}</p>
      )}
      {samples.length > 0 ? (
        <>
          <h4 className="bk-h3">{t('Passages')}</h4>
          <ul className="bk-example-list">
            {samples.map((x) => (
              <li key={x.coding_id}>
                <SegmentQuote text={x.text} sourceId={x.source_id} region={x.region} page={x.page} max={300} color={code.color} />
                <p className="muted">
                  {x.source_name} · {x.coder_name}
                </p>
                <Button onPress={() => void openAt(x.source_id, x.start, x.end, x.coding_id)}>{t('Open at source')}</Button>
              </li>
            ))}
          </ul>
          {profile.coding_ids.length > samples.length ? <p className="muted">{t('The first {count} of {count2}.', { count: samples.length, count2: num(profile.coding_ids.length) })}</p> : null}
        </>
      ) : null}
    </section>
  );
}
