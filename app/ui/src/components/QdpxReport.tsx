import type { QdpxReport } from '../types';
import { t } from '../i18n';

/**
 * What went into a QDPX file, or came out of one, and what did not. The REFI-QDA standard asks a program to tell the researcher at the
 * point of export and of import which parts of a project are discarded, in as much detail as it can (decision 159).
 */
export function QdpxReportView(props: { report: QdpxReport; direction: 'out' | 'in' }) {
  const { report, direction } = props;
  return (
    <div className="qdpx-report" data-testid="qdpx-report">
      <h3 className="stats-h5">{direction === 'out' ? t('What is in the file') : t('What came in')}</h3>
      <ul data-testid="qdpx-carried">
        {report.carried.map((line, i) => (
          <li key={i}>{line}</li>
        ))}
      </ul>
      <h3 className="stats-h5">{direction === 'out' ? t('What is not in the file') : t('What did not come in')}</h3>
      {report.left_out.length === 0 ? (
        <p>{t('Nothing was left out.')}</p>
      ) : (
        <ul data-testid="qdpx-left-out">
          {report.left_out.map((line, i) => (
            <li key={i}>{line}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
