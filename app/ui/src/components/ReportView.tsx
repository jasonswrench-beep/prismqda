import { Button } from 'react-aria-components';
import { appStore } from '../state/app';
import { t } from '../i18n';

/** Report: take work out of PrismQDA as files other programs open. The reports that need more of the project come later. */
export function ReportView() {
  return (
    <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-report">
      <h2 id="h-report">{t('Report')}</h2>
      <p className="help">{t('Take your work out of PrismQDA as files that Excel, Word, and other programs open.')}</p>
      <div className="cards">
        <section aria-labelledby="h-rep-report" className="card">
          <h3 id="h-rep-report">{t('Report')}</h3>
          <p>{t('One Word document made from the project records: what the study holds, the codebook, agreement between coders, a draft methods appendix, and more. You choose the sections.')}</p>
          <p className="muted">{t('As Word.')}</p>
          <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'export', what: 'report' } })} data-testid="build-report">{t('Build a report…')}</Button>
        </section>
        <section aria-labelledby="h-rep-codebook" className="card">
          <h3 id="h-rep-codebook">{t('Codebook')}</h3>
          <p>{t('Every code with its definition, its rules, and how often it is used.')}</p>
          <p className="muted">{t('As Excel, Word, CSV, or the REFI-QDA codebook format (QDC).')}</p>
          <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'export', what: 'codebook' } })}>{t('Export the codebook…')}</Button>
        </section>
        <section aria-labelledby="h-rep-passages" className="card">
          <h3 id="h-rep-passages">{t('Coded passages')}</h3>
          <p>{t('The words you coded, with the code, who coded them, and which source they came from. All of them, or one code at a time.')}</p>
          <p className="muted">{t('As Excel, Word, or CSV.')}</p>
          <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'export', what: 'segments' } })}>{t('Export coded passages…')}</Button>
        </section>
      </div>
      <p className="muted">{t('The Statistics notebook is exported from the Statistics page (Export the notebook…) as Word, Excel, a web page, or an R script.')}</p>
    </main>
  );
}
