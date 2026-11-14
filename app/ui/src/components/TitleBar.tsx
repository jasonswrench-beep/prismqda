import { Button, Dialog, DialogTrigger, Heading, Menu, MenuItem, MenuTrigger, Popover, Separator } from 'react-aria-components';
import { formatWhen } from '../lib/eventWording';
import { isApplePlatform, shortcutLabel } from '../lib/platform';
import { initialsOf } from '../lib/provenance';
import { goTo } from '../state/actions';
import { markNoticesSeen, useNotices } from '../state/announce';
import { appStore, useApp } from '../state/app';
import { commandById, runCommand } from '../state/commands';
import { BrandMark } from './BrandMark';
import { Icon } from './Icon';
import { t } from '../i18n';

/**
 * The title bar of the open project, as in the screen specification book: the mark and name of the program, the open
 * project (its chevron opens the project menu), the project search, the notifications bell, the settings gear, and the
 * account that changes are attributed to. The window's own minimize, maximize, and close controls come from the system.
 */
export function TitleBar() {
  const project = useApp((s) => s.project);
  const coders = useApp((s) => s.coders);
  const workspace = useApp((s) => s.workspace);
  const me = coders.find((c) => c.id === project?.me) ?? null;
  const apple = isApplePlatform();
  const run = (id: string) => void runCommand(commandById(id));

  return (
    <header className="bk-titlebar">
      <h1 className="bk-brand">
        <BrandMark size={26} />
        <span className="bk-wordmark">{t('PrismQDA')}</span>
      </h1>
      <span className="bk-divider" aria-hidden="true" />

      <MenuTrigger>
        <Button className="bk-project" aria-label={project?.sample_id ? t('{name}, a sample project, the open project. Press for the project menu', { name: project?.name ?? t('Project') }) : t('{name}, the open project. Press for the project menu', { name: project?.name ?? t('Project') })} data-testid="project-name">
          <span className="bk-project-name" title={project?.path}>
            {project?.name}
          </span>
          {project?.sample_id ? <span className="tag" title={t('A copy of a sample study. It is yours to change; the bundled study stays as it is.')}>{t('Sample')}</span> : null}
          {project?.read_only ? <span className="tag">{t('read only')}</span> : null}
          <Icon name="ui-chevron-down" size={16} />
        </Button>
        <Popover className="popover" placement="bottom start">
          <Menu className="menu" aria-label={t('Project menu')} onAction={(key) => run(String(key))}>
            <MenuItem id="go-home" className="menu-item">{t('Project overview')}</MenuItem>
            <MenuItem id="undo" className="menu-item" isDisabled={!!commandById('undo').unavailable(appStore.get())}>{t('Undo the last change')}</MenuItem>
            <MenuItem id="backup" className="menu-item" isDisabled={!!commandById('backup').unavailable(appStore.get())}>{t('Save a backup copy')}</MenuItem>
            <MenuItem id="new-project" className="menu-item">{t('New project…')}</MenuItem>
            <MenuItem id="open-project" className="menu-item">{t('Open another project…')}</MenuItem>
            <MenuItem id="shortcuts" className="menu-item">{t('Keyboard shortcuts')}</MenuItem>
            <MenuItem id="close-project" className="menu-item">{t('Close project')}</MenuItem>
          </Menu>
        </Popover>
      </MenuTrigger>

      <Button className="bk-search" onPress={() => appStore.set({ dialog: { kind: 'palette' } })} aria-label={t('Search project, or run a command')} data-testid="search-trigger">
        <Icon name="cmd-search" size={18} />
        <span className="bk-search-text">{t('Search project ({shortcut})', { shortcut: shortcutLabel('k', apple) })}</span>
      </Button>

      <div className="bk-title-actions">
        <Notifications />
        <Button className={`bk-icon-button${workspace === 'settings' ? ' current' : ''}`} aria-label={t('Settings')} aria-current={workspace === 'settings' ? 'page' : undefined} onPress={() => goTo('settings')}>
          <Icon name="ws-settings" size={20} />
        </Button>
        <DialogTrigger>
          <Button className="bk-account" aria-label={me ? t('Account: {name}. Changes you make are attributed to this person', { name: me.name }) : t('Account. Changes you make are attributed to this person')}>
            <span className="bk-avatar" aria-hidden="true">
              {me ? initialsOf(me.name) : <Icon name="ui-user" size={16} />}
            </span>
            <Icon name="ui-chevron-down" size={14} />
          </Button>
          <Popover className="popover bk-popover" placement="bottom end">
            <Dialog className="bk-menu-dialog" aria-label={t('Account')}>
              <p className="bk-account-name">
                <span className="bk-avatar" aria-hidden="true">
                  {me ? initialsOf(me.name) : <Icon name="ui-user" size={16} />}
                </span>
                <span>
                  <strong>{me?.name ?? t('No one is coding')}</strong>
                  <span className="muted bk-account-role">{me ? (me.role === 'model' ? t('AI model') : t('Researcher')) : t('Read-only project')}</span>
                </span>
              </p>
              <Separator className="bk-sep" />
              <p className="muted">
                {me ? t('Every change you make is recorded as {name}, with the time.', { name: me.name }) : t('This project is open read only, so nothing can be changed or attributed.')}
              </p>
              <p className="muted">{t('The project, the person coding, and whether the file is saved are three separate things; the status bar shows the file.')}</p>
            </Dialog>
          </Popover>
        </DialogTrigger>
      </div>
    </header>
  );
}

/** The bell: what the program has said lately, and a dot when something went wrong since you last looked. */
function Notifications() {
  const { items, unread } = useNotices();
  return (
    <DialogTrigger onOpenChange={(open) => open && markNoticesSeen()}>
      <Button className="bk-icon-button" aria-label={unread > 0 ? t('Notifications, {unread} new', { unread }) : t('Notifications')}>
        <Icon name="ui-bell" size={20} />
        {unread > 0 ? <span className="bk-dot" aria-hidden="true" /> : null}
      </Button>
      <Popover className="popover bk-popover" placement="bottom end">
        <Dialog className="bk-menu-dialog bk-notices" aria-label={t('Notifications')}>
          <Heading slot="title" className="bk-popover-title">{t('Notifications')}</Heading>
          {items.length === 0 ? (
            <p className="muted">{t('Nothing yet. What PrismQDA tells you while you work is kept here for a while.')}</p>
          ) : (
            <ul className="bk-notice-list">
              {items.map((n) => (
                <li key={n.id} className={n.mode === 'assertive' ? 'problem' : undefined}>
                  <span>{n.text}</span>
                  <span className="muted">{formatWhen(new Date(n.at).toISOString())}</span>
                </li>
              ))}
            </ul>
          )}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}
