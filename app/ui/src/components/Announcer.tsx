import { Button } from 'react-aria-components';
import { formatWhen } from '../lib/eventWording';
import { scopeChip } from '../lib/retrieval';
import { saveProject, showPassages } from '../state/actions';
import { useAnnouncements } from '../state/announce';
import { useApp } from '../state/app';
import { Icon } from './Icon';
import { rich, t } from '../i18n';

/**
 * The status bar along the bottom of the window. On the left, the scope that is filtering the results and the jobs that
 * are running; on the right, whether the file is saved, and where the project lives. In the middle, the last thing the
 * program said. The polite part is a live region, so a screen reader reads each new message; the assertive part is for
 * errors and interrupts. Both are also visible, so people who do not use a screen reader see the same words.
 */
export function Announcer() {
  const { polite, assertive } = useAnnouncements();
  const phase = useApp((s) => s.phase);
  const inWorkspace = phase === 'workspace';
  return (
    <footer className="statusbar bk-status" aria-label={t('Status')}>
      {inWorkspace ? <StatusLeft /> : null}
      <div className="bk-status-message">
        <div role="status" aria-live="polite" aria-atomic="true" className="status-polite" data-testid="status">
          {polite}
        </div>
        <div role="alert" aria-live="assertive" aria-atomic="true" className="status-assertive" data-testid="alert">
          {assertive}
        </div>
      </div>
      {inWorkspace ? <StatusRight /> : null}
    </footer>
  );
}

/** Filters (the scope on the results) and Jobs (background work: none exists yet). */
function StatusLeft() {
  const chosen = useApp((s) => s.activeCodeIds.length);
  const mode = useApp((s) => s.retrievalMode);
  const filtered = chosen > 0 || mode !== 'all';
  return (
    <ul className="status-facts bk-status-left" aria-label={t('Scope and jobs')}>
      <li>
        <Button className="bk-status-button" onPress={() => showPassages()} aria-label={filtered ? t('Filters: {filters}. Press to go to the filters', { filters: scopeChip(chosen, mode) }) : t('Filters: None. Press to go to the filters')}>
          <Icon name="ui-filter" size={15} />
          <span>{filtered ? rich('Filters: <strong>{filters}</strong>', { filters: scopeChip(chosen, mode), strong: (inside) => <strong>{inside}</strong> }) : rich('Filters: <strong>None</strong>', { strong: (inside) => <strong>{inside}</strong> })}
          </span>
        </Button>
      </li>
      <li>
        <Icon name="st-job" size={15} />
        <span>{rich('Jobs: <strong>none running</strong>', { strong: (inside) => <strong>{inside}</strong> })}
        </span>
      </li>
    </ul>
  );
}

/** Saved (pending, saving, saved, or failed, with the time) and where the project is kept. */
function StatusRight() {
  const project = useApp((s) => s.project);
  const state = useApp((s) => s.saveState);
  const at = useApp((s) => s.savedAt);
  const error = useApp((s) => s.saveError);
  const when = at ? formatWhen(new Date(at).toISOString()) : null;
  return (
    <ul className="status-facts bk-status-right" aria-label={t('Project state')}>
      <li data-testid="save-state" data-state={project?.read_only ? 'read-only' : state}>
        {project?.read_only ? (
          <>
            <Icon name="st-info" size={15} />
            <span>{t('Read only')}</span>
          </>
        ) : state === 'saving' ? (
          <>
            <Icon name="st-saving" size={15} />
            <span>{t('Saving…')}</span>
          </>
        ) : state === 'failed' ? (
          <>
            <Icon name="st-save-failed" size={15} />
            <span>{error ? t('Not saved: {error} ', { error }) : t('Not saved ')}
              <Button className="bk-status-button link" onPress={() => saveProject()}>{t('Why?')}</Button>
            </span>
          </>
        ) : (
          <>
            <Icon name="st-saved" size={15} className="saved-icon" />
            <span>
              {when ? rich('<strong>Saved</strong> ({when})', { when, strong: (inside) => <strong>{inside}</strong> }) : rich('<strong>Saved</strong>', { strong: (inside) => <strong>{inside}</strong> })}
            </span>
          </>
        )}
      </li>
      {project?.encrypted ? (
        <li data-testid="encrypted-badge">
          <Icon name="st-encrypted" size={15} />
          <span>{t('Encrypted')}</span>
        </li>
      ) : null}
      <li>
        <Icon name="set-storage" size={15} />
        {rich('<span><strong>Local project</strong></span><x1/><span2>{offline}</span2>', { offline: project?.offline_lock ? t('Offline lock on') : t('Offline'), span: (inside) => <span>{inside}</span>, strong: (inside) => <strong>{inside}</strong>, x1: <Icon name="st-offline" size={15} />, span2: (inside) => <span>{inside}</span> })}
      </li>
    </ul>
  );
}
