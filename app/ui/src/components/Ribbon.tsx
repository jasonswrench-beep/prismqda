import { Button, Group, Menu, MenuItem, MenuTrigger, Popover, Tab, TabList, TabPanel, Tabs, Toolbar } from 'react-aria-components';
import { setRibbonTab } from '../state/actions';
import { announce } from '../state/announce';
import { useApp, type AppState, type RibbonTabId } from '../state/app';
import { COMMANDS, NOT_BUILT_TABS, RIBBON, RIBBON_TABS, commandById, runCommand, shortcutFor, type Command, type RibbonItem } from '../state/commands';
import { Icon } from './Icon';
import { t } from '../i18n';

/**
 * The row of tabs and the commands under the tab that is chosen. A tab changes which commands are on show and nothing
 * else: the project, the open documents, and the selection stay as they are (screen specification book, Home ribbon).
 *
 * The tabs are a tab list, so the arrow keys move between them. The commands are one toolbar and so one tab stop; the
 * arrow keys move between the commands. A command that cannot run now stays where it is and says why.
 */
export function RibbonArea() {
  const tab = useApp((s) => s.ribbonTab);
  // The tabs and the commands are one landmark, so screen reader users can jump to the ribbon and no control is outside one.
  return (
    <div role="region" aria-label={t('Command ribbon')} className="bk-ribbon-landmark">
      <Tabs selectedKey={tab} onSelectionChange={(k) => setRibbonTab(k as RibbonTabId)} className="bk-tabs">
        <TabList aria-label={t('Ribbon tabs')} className="bk-tablist">
          {RIBBON_TABS.map((t) => (
            <Tab key={t.id} id={t.id} className="bk-tab">
              {t.label}
            </Tab>
          ))}
        </TabList>
        {RIBBON_TABS.map((t) => (
          <TabPanel key={t.id} id={t.id} className="bk-ribbon-panel" shouldForceMount={false}>
            <Ribbon tab={t.id} />
          </TabPanel>
        ))}
      </Tabs>
    </div>
  );
}

function Ribbon({ tab }: { tab: RibbonTabId }) {
  const state = useApp((s) => s);
  const groups = RIBBON[tab];
  const note = NOT_BUILT_TABS[tab];
  return (
    <div className="ribbon-region bk-ribbon">
      {groups.length === 0 ? (
        <p className="bk-ribbon-note" role="status">
          {note ?? t('Nothing here yet.')}
        </p>
      ) : (
        <Toolbar aria-label={t('Commands')} className="bk-ribbon-row">
          {groups.map((g, gi) => (
            <Group key={`${g.caption}-${gi}`} aria-label={g.caption || t('Documents')} className="bk-group">
              <div className="bk-group-buttons">
                {g.items.map((item) => (
                  <RibbonSlot key={typeof item === 'string' ? item : item.id} item={item} state={state} />
                ))}
              </div>
              <span className="bk-group-caption" aria-hidden="true">
                {g.caption}
              </span>
            </Group>
          ))}
        </Toolbar>
      )}
    </div>
  );
}

function RibbonSlot({ item, state }: { item: RibbonItem; state: AppState }) {
  if (typeof item === 'string') return <RibbonButton command={commandById(item)} state={state} />;
  const main = commandById(item.id);
  const reason = main.unavailable(state);
  const menu = item.menu.map(commandById);
  return (
    <span className="bk-split">
      <RibbonButton command={main} state={state} />
      <MenuTrigger>
        <Button className="bk-chevron" aria-label={t('More ways to use {label}', { label: main.label })} isDisabled={false}>
          <Icon name="ui-chevron-down" size={14} />
        </Button>
        <Popover className="popover" placement="bottom start">
          <Menu className="menu" aria-label={t('{label} options', { label: main.label })} onAction={(key) => void runCommand(COMMANDS.find((c) => c.id === key)!)}>
            {menu.map((c) => (
              <MenuItem key={c.id} id={c.id} textValue={c.label} className="menu-item" isDisabled={!!c.unavailable(state)}>
                {c.title}
              </MenuItem>
            ))}
          </Menu>
        </Popover>
      </MenuTrigger>
    </span>
  );
}

/** The buttons whose names the book breaks over two lines, as it draws them; the others stay on one line. */
const LABEL_LINES: Record<string, string[]> = {
  get 'new-project'() { return [t('New'), t('Project')]; },
  get 'open-project'() { return [t('Open'), t('Project')]; },
  get import() { return [t('Add'), t('Documents')]; },
  get 'new-code'() { return [t('New'), t('Code')]; },
  get memo() { return [t('New'), t('Memo')]; },
  get 'new-case'() { return [t('New'), t('Case')]; },
  get 'apply-code'() { return [t('Code'), t('Selection')]; },
  get 'auto-code'() { return [t('Auto'), t('Code')]; },
  get 'review-queue'() { return [t('Review'), t('Queue')]; },
  get 'repeat-code'() { return [t('Quick Code')]; },
  get 'code-summary'() { return [t('Code'), t('Summary')]; },
  get 'code-relations'() { return [t('Code'), t('Relations')]; },
  get 'compare-cases'() { return [t('Compare'), t('Cases')]; },
  get 'report-builder'() { return [t('Report'), t('Builder')]; },
};

function RibbonButton({ command, state }: { command: Command; state: AppState }) {
  const reason = command.unavailable(state);
  const shown = command.dynamicLabel ? command.dynamicLabel(state) : command.label;
  const name = command.dynamicLabel ? t('Scope: {label}', { label: shown }) : command.label;
  const key = shortcutFor(command);
  const showKey = !!key && !/^[A-Z]$/.test(key);
  const why = reason ? `${command.title}. ${reason}` : key ? `${command.title} (${key})` : command.title;
  const whyId = `ribbon-why-${command.id}`;
  return (
    <span className="ribbon-slot" title={why}>
      <Button
        className={`bk-button${reason ? ' unavailable' : ''}`}
        aria-disabled={reason ? 'true' : undefined}
        aria-label={showKey ? `${name} (${key})` : name}
        aria-describedby={reason ? whyId : undefined}
        data-command={command.id}
        onPress={() => {
          if (reason) announce(t('{title} is not available right now. {reason}', { title: command.title, reason }));
          else void runCommand(command);
        }}
      >
        <Icon name={command.icon} size={24} />
        <span className="bk-label" aria-hidden="true">
          {(LABEL_LINES[command.id] ?? [shown]).map((line, i) => (
            <span key={i} className="bk-line">
              {line}
            </span>
          ))}
          {showKey ? <span className="bk-line bk-key">({key})</span> : null}
        </span>
      </Button>
      {reason ? (
        <span id={whyId} className="visually-hidden">
          {reason}
        </span>
      ) : null}
    </span>
  );
}
