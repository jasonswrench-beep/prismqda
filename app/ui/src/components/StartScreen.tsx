import { useEffect, useState } from 'react';
import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import fileProject from '../../../../assets/brand/file-project.svg';
import { call, toApiError } from '../bridge';
import { describeWhen, fileNameOf, type RecentEntry } from '../lib/recent';
import { openProject, removeRecentProject } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { FileStat } from '../types';
import { BrandMark } from './BrandMark';
import { GetStarted } from './GetStarted';
import { Icon, type IconName } from './Icon';
import { NewProjectForm } from './NewProjectForm';
import { SettingsView } from './SettingsView';
import { Notice } from './common';
import { rich, t } from '../i18n';

/** The rail on the left of the launcher: the destinations of the whole program, with the ones that need a project saying so. */
const RAIL: { id: string; label: string; icon: IconName; active: IconName; needsProject: boolean }[] = [
  { id: 'home', get label() { return t('Home'); }, icon: 'ws-home', active: 'ws-home-active', needsProject: false },
  { id: 'data', get label() { return t('Data'); }, icon: 'ws-data', active: 'ws-data-active', needsProject: true },
  { id: 'code', get label() { return t('Code'); }, icon: 'ws-code', active: 'ws-code-active', needsProject: true },
  { id: 'explore', get label() { return t('Explore'); }, icon: 'ws-explore', active: 'ws-explore-active', needsProject: true },
  { id: 'text', get label() { return t('Text Analytics'); }, icon: 'ws-text', active: 'ws-text-active', needsProject: true },
  { id: 'stats', get label() { return t('Statistics'); }, icon: 'ws-stats', active: 'ws-stats-active', needsProject: true },
  { id: 'synthesize', get label() { return t('Synthesize'); }, icon: 'ws-synth', active: 'ws-synth-active', needsProject: true },
  { id: 'report', get label() { return t('Report'); }, icon: 'ws-report', active: 'ws-report-active', needsProject: true },
  { id: 'team', get label() { return t('Team'); }, icon: 'ws-team', active: 'ws-team-active', needsProject: true },
  { id: 'settings', get label() { return t('Settings'); }, icon: 'ws-settings', active: 'ws-settings-active', needsProject: false },
];

/**
 * The launcher, as in the screen specification book (Welcome and Open Project): a light title bar with the program's mark,
 * a rail of destinations, and a page of three ways to begin, the recent projects, and Get Started. It works offline and
 * never waits for anything: the recent list is read from this computer, and what is found out about each file arrives
 * when it can.
 */
export function StartScreen() {
  const page = useApp((s) => s.launcherPage);
  const bootError = useApp((s) => s.bootError);
  const goHome = () => appStore.set({ launcherPage: 'home' });

  return (
    <div className="bk-launcher">
      <header className="bk-titlebar">
        <h1 className="bk-brand">
          <BrandMark size={26} />
          <span className="bk-wordmark">{t('PrismQDA')}</span>
        </h1>
        {page === 'new' ? (
          <nav className="bk-crumbs" aria-label={t('You are here')}>
            <span className="bk-divider" aria-hidden="true" />
            <Button className="bk-crumb" onPress={goHome}>{t('Welcome')}</Button>
            {rich('<span>/</span><span2>New Project</span2>', { span: (inside) => <span aria-hidden="true">{inside}</span>, span2: (inside) => <span aria-current="page">{inside}</span> })}
          </nav>
        ) : null}
      </header>
      <div className="bk-launcher-body">
        <nav className="bk-rail" aria-label={t('Destinations')}>
          <ul>
            {RAIL.map((r) => {
              const current = (r.id === 'home' && page !== 'settings') || (r.id === 'settings' && page === 'settings');
              const gap = r.id === 'team';
              return (
                <li key={r.id} className={gap ? 'gap' : undefined}>
                  <Button
                    className={`bk-rail-item${current ? ' current' : ''}${r.needsProject ? ' unavailable' : ''}`}
                    aria-current={current ? 'page' : undefined}
                    aria-disabled={r.needsProject ? 'true' : undefined}
                    aria-describedby={r.needsProject ? 'rail-why' : undefined}
                    onPress={() => {
                      if (r.needsProject) announce(t('{label} needs a project. Create one, open one, or try a sample first.', { label: r.label }));
                      else appStore.set({ launcherPage: r.id === 'settings' ? 'settings' : 'home' });
                    }}
                  >
                    <Icon name={current ? r.active : r.icon} size={20} />
                    {r.label}
                  </Button>
                </li>
              );
            })}
          </ul>
          <p id="rail-why" className="visually-hidden">{t('Needs an open project.')}</p>
        </nav>
        {page === 'settings' ? (
          <SettingsView />
        ) : (
          <main id="main-content" tabIndex={-1} className="bk-launcher-main" aria-labelledby={page === 'new' ? 'np-title' : 'welcome-title'}>
            {bootError ? <Notice>{bootError}</Notice> : null}
            {page === 'new' ? (
              <div className="bk-np-page">
                <NewProjectForm onCancel={goHome} />
              </div>
            ) : (
              <Welcome />
            )}
          </main>
        )}
      </div>
    </div>
  );
}

function Welcome() {
  return (
    <div className="bk-welcome">
      <div className="bk-welcome-art" aria-hidden="true" />
      <h2 id="welcome-title" className="bk-welcome-title">{t('Welcome to PrismQDA')}</h2>
      <p className="bk-sub">{t('Qualitative, quantitative, and mixed methods analysis in one project.')}</p>
      <div className="bk-actions" role="group" aria-label={t('Ways to begin')}>
        <Button className="bk-action primary" onPress={() => appStore.set({ launcherPage: 'new' })}>
          <Icon name="ui-plus" size={30} />
          <span>{t('New Project')}</span>
        </Button>
        <Button className="bk-action" onPress={() => appStore.set({ dialog: { kind: 'open-project' } })}>
          <Icon name="src-folder" size={30} />
          <span>{t('Open Project')}</span>
        </Button>
        <Button className="bk-action" onPress={() => appStore.set({ dialog: { kind: 'sample-projects' } })}>
          <Icon name="ui-graduation-cap" size={30} />
          <span>{t('Sample Projects')}</span>
        </Button>
        <Button className="bk-action" onPress={() => appStore.set({ dialog: { kind: 'import-qdpx' } })} data-testid="welcome-import-qdpx">
          <Icon name="cmd-import" size={30} />
          <span>{t('Import QDPX')}</span>
        </Button>
      </div>
      <div className="bk-welcome-grid">
        <RecentProjects />
        <GetStarted inProject={false} />
      </div>
    </div>
  );
}

/** Recent Projects: each row opens the project (the same checks as Open Project), and has its own menu. */
function RecentProjects() {
  const recent = useApp((s) => s.recent);
  const [stats, setStats] = useState<Record<string, FileStat | null>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  // What is true of each file now (is it there, and when was it changed), asked without opening it.
  useEffect(() => {
    let live = true;
    for (const r of recent) {
      call('file.stat', { path: r.path })
        .then((s) => live && setStats((prev) => ({ ...prev, [r.path]: s })))
        .catch(() => live && setStats((prev) => ({ ...prev, [r.path]: null })));
    }
    return () => {
      live = false;
    };
  }, [recent]);

  const open = async (r: RecentEntry) => {
    setError(null);
    setBusy(r.path);
    try {
      await openProject(r.path);
    } catch (e) {
      setBusy(null);
      const err = toApiError(e);
      const gone = stats[r.path]?.exists === false || /not exist|not found|cannot find|No such/i.test(err.message);
      setError(gone ? t('{name} was not found at {path}. Use the menu beside it to find it.', { name: r.name, path: r.path }) : t('{name} could not be opened: {message}', { name: r.name, message: err.message }));
    }
  };

  const when = (r: RecentEntry): string => {
    const s = stats[r.path];
    if (s && s.exists === false) return t('File not found at its saved place');
    if (s?.modified) return t('Modified {when}', { when: describeWhen(Date.parse(s.modified), Date.now()) });
    return r.openedAt > 0 ? t('Opened {when}', { when: describeWhen(r.openedAt, Date.now()) }) : '';
  };

  return (
    <section className="bk-card bk-recent" aria-labelledby="recent-title">
      <h2 id="recent-title" className="bk-card-title">{t('Recent Projects')}</h2>
      {error ? <Notice>{error}</Notice> : null}
      {recent.length === 0 ? (
        <p className="muted bk-empty">{t('No recent projects yet. Start a new project, open one, or practice on a sample above.')}</p>
      ) : (
        <ul className="bk-recent-list">
          {recent.map((r) => (
            <li key={r.path} className="bk-recent-row">
              <Button className="bk-recent-open" isDisabled={busy === r.path} onPress={() => void open(r)} aria-label={t('Open {file}. {when}', { file: fileNameOf(r.path), when: `${when(r)}${stats[r.path]?.encrypted ? `. ${t('Encrypted')}` : ''}` })}>
                <img src={fileProject} alt="" width={30} height={30} className="bk-file-badge" />
                <span className="bk-recent-text">
                  <strong>{fileNameOf(r.path)}</strong>
                  <span className={`muted${stats[r.path]?.exists === false ? ' missing' : ''}`}>{when(r)}{stats[r.path]?.encrypted ? `. ${t('Encrypted')}` : ''}</span>
                </span>
              </Button>
              <MenuTrigger>
                <Button className="icon-button" aria-label={t('More options for {name}', { name: r.name })}>
                  <Icon name="ui-more" size={20} />
                </Button>
                <Popover className="popover" placement="bottom end">
                  <Menu
                    className="menu"
                    aria-label={t('Options for {name}', { name: r.name })}
                    onAction={(key) => {
                      if (key === 'open') void open(r);
                      else if (key === 'locate') appStore.set({ dialog: { kind: 'locate-project', path: r.path, name: r.name } });
                      else if (key === 'remove') removeRecentProject(r.path);
                    }}
                  >
                    <MenuItem id="open" className="menu-item">{t('Open')}</MenuItem>
                    <MenuItem id="locate" className="menu-item">{t('Locate…')}</MenuItem>
                    <MenuItem id="remove" className="menu-item">{t('Remove from recent list')}</MenuItem>
                  </Menu>
                </Popover>
              </MenuTrigger>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
