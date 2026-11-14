import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components';
import { goExplore, goTo } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp, type DialogState } from '../state/app';
import { Icon, type IconName } from './Icon';
import { t } from '../i18n';

interface Item {
  id: string;
  label: string;
  icon: IconName;
  /** True only when the person has done the thing; nothing is ticked just for opening a screen. */
  done: boolean;
  /** What pressing it does inside an open project. */
  run: () => void;
}

/**
 * Get Started: the five steps of a first study. Before a project is open each one offers the three ways to begin (a new
 * project, an existing one, or a sample); once a project is open each one goes to its screen, and a step is ticked only
 * when the project shows it was done (documents added, codes made, passages coded).
 */
export function GetStarted(props: { inProject: boolean }) {
  const sources = useApp((s) => s.sources);
  const codes = useApp((s) => s.codes);
  const coded = sources.reduce((n, s) => n + s.codings, 0);
  const readOnly = useApp((s) => s.project?.read_only ?? false);

  const items: Item[] = [
    { id: 'import', label: t('Import your data'), icon: 'cmd-import', done: sources.length > 0, run: () => goTo('import') },
    { id: 'codebook', label: t('Create a codebook'), icon: 'cmd-new-code', done: codes.length > 0, run: () => appStore.set({ dialog: { kind: 'new-code', parentId: null } }) },
    {
      id: 'code',
      label: t('Code your first document'),
      icon: 'cmd-code',
      done: coded > 0,
      run: () => {
        if (sources.length === 0) {
          announce(t('Add a document first, then select words and apply a code.'));
          goTo('import');
        } else goTo('code');
      },
    },
    { id: 'explore', label: t('Explore and analyze'), icon: 'exp-query', done: false, run: () => goExplore('query') },
    { id: 'report', label: t('Build a report'), icon: 'rep-outline', done: false, run: () => goTo('report') },
  ];

  return (
    <section className="bk-card bk-getstarted" aria-labelledby="gs-title">
      <h2 id="gs-title" className="bk-card-title">{t('Get Started')}</h2>
      <ol className="bk-steps">
        {items.map((it) => {
          const body = (
            <>
              <Icon name={it.icon} size={20} className="bk-step-icon" />
              <span>{it.label}</span>
              {it.done ? (
                <>
                  <Icon name="ui-check" size={16} className="bk-step-done" />
                  <span className="visually-hidden">{t('Done')}</span>
                </>
              ) : null}
            </>
          );
          return (
            <li key={it.id} className={it.done ? 'done' : undefined}>
              {props.inProject ? (
                <Button className="bk-step" isDisabled={readOnly && (it.id === 'import' || it.id === 'codebook')} onPress={it.run} data-step={it.id}>
                  {body}
                </Button>
              ) : (
                <MenuTrigger>
                  <Button className="bk-step" data-step={it.id} aria-label={t('{label}. Choose how to begin', { label: it.label })}>
                    {body}
                  </Button>
                  <Popover className="popover" placement="bottom start">
                    <Menu
                      className="menu"
                      aria-label={t('{label}: how to begin', { label: it.label })}
                      onAction={(key) => appStore.set({ dialog: { kind: key === 'sample' ? 'sample-projects' : key === 'open' ? 'open-project' : 'new-project' } as DialogState })}
                    >
                      <MenuItem id="new" className="menu-item">{t('Start a new project')}</MenuItem>
                      <MenuItem id="open" className="menu-item">{t('Open a project')}</MenuItem>
                      <MenuItem id="sample" className="menu-item">{t('Practice on a sample project')}</MenuItem>
                    </Menu>
                  </Popover>
                </MenuTrigger>
              )}
            </li>
          );
        })}
      </ol>
    </section>
  );
}
