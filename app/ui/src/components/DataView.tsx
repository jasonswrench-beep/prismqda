import { useEffect, useState } from 'react';
import { Button } from 'react-aria-components';
import { call } from '../bridge';
import { describeCoverage } from '../lib/coverage';
import { goTo, openAt } from '../state/actions';
import { appStore, useApp } from '../state/app';
import type { CaseRow } from '../types';
import { Icon } from './Icon';
import { SOURCE_TYPE_ICONS, SOURCE_TYPE_LABELS, plural } from './common';
import { num, t, tn } from '../i18n';

/** A project can have tens of thousands of cases (one per survey respondent), so only the first of them are drawn here. */
const SHOWN_CASES = 50;

/** Data: what has been brought into the project, as plain tables. Opening a source takes you to it in Code. */
export function DataView() {
  const sources = useApp((s) => s.sources);
  const coverage = useApp((s) => s.coverage);
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const [cases, setCases] = useState<CaseRow[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    void call('case.list')
      .then((c) => !cancelled && setCases(c))
      .catch(() => !cancelled && setCases([]));
    return () => {
      cancelled = true;
    };
  }, [version, sources.length]);

  return (
    <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-data">
      <h2 id="h-data">{t('Data')}</h2>
      <p className="help">{t('Everything brought into this project. Choose a source to read and code it.')}</p>
      <div className="actions">
        <Button className="primary" isDisabled={readOnly} onPress={() => goTo('import')}>{t('Import a file')}</Button>
        <Button isDisabled={readOnly} onPress={() => appStore.set({ dialog: { kind: 'import-folder' } })}>{t('Import a whole folder')}</Button>
      </div>

      <section aria-labelledby="h-data-sources" className="card">
        <h3 id="h-data-sources">{t('Sources ({count})', { count: num(sources.length) })}</h3>
        {sources.length === 0 ? (
          <p className="muted">{t('No sources yet. Import an interview, a document, or a spreadsheet of survey answers to begin.')}</p>
        ) : (
          <div className="table-scroll">
            <table className="data">
              <caption className="visually-hidden">{t('Sources in this project')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Name')}</th>
                  <th scope="col">{t('Kind')}</th>
                  <th scope="col">{t('Folder')}</th>
                  <th scope="col" className="num">{t('Units')}</th>
                  <th scope="col" className="num">{t('Characters')}</th>
                  <th scope="col" className="num">{t('Codings')}</th>
                  <th scope="col">{t('Coded')}</th>
                </tr>
              </thead>
              <tbody>
                {sources.map((s) => (
                  <tr key={s.id}>
                    <th scope="row">
                      <Button className="link-button" onPress={() => void openAt(s.id, 0, 0)}>
                        {s.name}
                      </Button>
                    </th>
                    <td>
                      <span className="with-icon">
                        {SOURCE_TYPE_ICONS[s.source_type] ? <Icon name={SOURCE_TYPE_ICONS[s.source_type]!} size={16} /> : null}
                        {SOURCE_TYPE_LABELS[s.source_type] ?? s.source_type}
                      </span>
                    </td>
                    <td>{s.folder === '' ? t('No folder') : s.folder}</td>
                    <td className="num">{num(s.units)}</td>
                    <td className="num">{num(s.characters)}</td>
                    <td className="num">{num(s.codings)}</td>
                    <td>{describeCoverage(coverage[s.id] ?? null, s.codings)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section aria-labelledby="h-data-cases" className="card">
        <h3 id="h-data-cases">{t('Cases ({count})', { count: num(cases?.length ?? 0) })}</h3>
        {cases === null ? (
          <p className="muted" role="status">{t('Loading…')}</p>
        ) : cases.length === 0 ? (
          <p className="muted">{t('No cases yet. A case is the person, site, or thread you compare. Importing a spreadsheet with a column for it creates them.')}</p>
        ) : (
          <div className="table-scroll">
            <table className="data">
              <caption className="visually-hidden">{t('Cases in this project')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Case')}</th>
                  <th scope="col" className="num">{t('Sources')}</th>
                  <th scope="col" className="num">{t('Units')}</th>
                </tr>
              </thead>
              <tbody>
                {cases.slice(0, SHOWN_CASES).map((c) => (
                  <tr key={c.id}>
                    <th scope="row">{c.name}</th>
                    <td className="num">{tn(c.sources, { one: '{count} source', other: '{count} sources' })}</td>
                    <td className="num">{tn(c.units, { one: '{count} unit', other: '{count} units' })}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {cases !== null && cases.length > SHOWN_CASES ? (
          <p className="muted">{t('Showing the first {SHOWN_CASES} of {count} cases.', { SHOWN_CASES, count: num(cases.length) })}</p>
        ) : null}
      </section>
    </main>
  );
}
