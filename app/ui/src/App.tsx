import { Fragment, useEffect } from 'react';
import { Announcer } from './components/Announcer';
import { CodeDialogs } from './components/CodeDialogsHost';
import { HelpDialog } from './components/HelpDialog';
import { AddLanguageDialog, TranslatorFileDialog } from './components/LanguageDialogs';
import { NewCaseDialog } from './components/CaseDialog';
import { PassphraseDialog } from './components/PassphraseDialog';
import { LinkPassageDialog } from './components/PassageLinks';
import { NewThemeDialog } from './components/NewThemeDialog';
import { WorkingSetDialog } from './components/WorkingSet';
import { QdcImportDialog } from './components/QdcImportDialog';
import { QdpxImportDialog } from './components/QdpxImportDialog';
import { NewProjectDialog, OpenProjectDialog, SampleProjectsDialog } from './components/ProjectDialogs';
import { CodePickerDialog } from './components/CodePickerDialog';
import { CommandPalette } from './components/CommandPalette';
import { Frame } from './components/Frame';
import { ExportDialog } from './components/ExportDialog';
import { ImportFolderDialog } from './components/ImportFolderDialog';
import { Splash } from './components/Splash';
import { StartScreen } from './components/StartScreen';
import { interpretAreaKey, undoesTyping } from './lib/shortcuts';
import { initApp, undoLast } from './state/actions';
import { announce } from './state/announce';
import { watchLaunchPaths } from './state/launch';
import { appStore, focusPane, useApp } from './state/app';
import { commandById, runCommand } from './state/commands';
import { t, useLocale } from './i18n';

const AREAS = ['pane-sources', 'codebook-section', 'main-content', 'pane-retrieved', 'pane-details'];
const AREA_NAMES: Record<string, string> = {
  get ['pane-sources']() { return t('Sources'); },
  get ['main-content']() { return t('the source text'); },
  get ['pane-retrieved']() { return t('Passages'); },
  get ['codebook-section']() { return t('Codebook'); },
  get ['pane-details']() { return t('Details'); },
};

export function App() {
  const phase = useApp((s) => s.phase);
  const dialog = useApp((s) => s.dialog);
  // When the language changes, everything below is drawn again from the start (the key), but this component is not, so the
  // start-up below does not run a second time and the person stays where they were.
  const locale = useLocale();

  const projectName = useApp((s) => s.project?.name);
  useEffect(() => {
    // A project file double clicked in Explorer (or sent by a second start) is opened once the window has started.
    let stop = () => {};
    let ended = false;
    void initApp().then(async () => {
      const off = await watchLaunchPaths();
      if (ended) off();
      else stop = off;
    });
    return () => {
      ended = true;
      stop();
    };
  }, []);

  // The window title names the project, so people who switch windows can tell them apart.
  useEffect(() => {
    document.title = projectName ? `${projectName} - PrismQDA` : 'PrismQDA';
  }, [projectName]);

  // F6 moves between the main areas and F1 opens the shortcut list, wherever the focus is.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = interpretAreaKey(e);
      if (!action) return;
      if (action === 'help') {
        e.preventDefault();
        if (appStore.get().phase === 'workspace') appStore.set({ dialog: { kind: 'help' } });
        return;
      }
      if (appStore.get().phase !== 'workspace' || appStore.get().workspace !== 'code' || appStore.get().dialog) return;
      e.preventDefault();
      const active = document.activeElement;
      const here = AREAS.findIndex((id) => {
        const el = document.getElementById(id);
        return el && active && el.contains(active);
      });
      const next = (here + (action === 'next' ? 1 : -1) + AREAS.length) % AREAS.length;
      focusPane(AREAS[next]);
      announce(t('Now in {value}.', { value: AREA_NAMES[AREAS[next]] }));
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  // Ctrl+K opens the search and command box; Ctrl+Z undoes, except inside a box where it undoes typing.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (appStore.get().phase !== 'workspace' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
      const key = e.key.toLowerCase();
      if (key === 'k') {
        e.preventDefault();
        appStore.set((st) => (st.dialog?.kind === 'palette' ? { dialog: null } : { dialog: { kind: 'palette' } }));
      } else if (key === 's' || key === 'd') {
        // Save and Quick Code, as the ribbon shows them. Quick Code is not taken from a box where Ctrl+D means something else.
        const el = document.activeElement as HTMLElement | null;
        const typing = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT');
        if (appStore.get().dialog || (key === 'd' && typing)) return;
        e.preventDefault();
        void runCommand(commandById(key === 's' ? 'save' : 'repeat-code'));
      } else if (key === 'z') {
        if (undoesTyping(document.activeElement as HTMLElement | null) || appStore.get().dialog) return;
        e.preventDefault();
        void undoLast();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  return (
    <Fragment key={locale}>
      <a
        className="skip-link"
        href="#main-content"
        onClick={(e) => {
          e.preventDefault();
          document.getElementById('main-content')?.focus();
        }}
      >{t('Skip to the main content')}</a>
      {phase === 'loading' || phase === 'blocked' ? (
        <Splash />
      ) : phase === 'start' ? (
        <StartScreen />
      ) : (
        <Frame />
      )}
      <Announcer />
      {dialog?.kind === 'import-folder' ? <ImportFolderDialog /> : null}
      {dialog?.kind === 'export' ? <ExportDialog /> : null}
      {dialog?.kind === 'help' ? <HelpDialog /> : null}
      {dialog?.kind === 'pick-code' ? <CodePickerDialog /> : null}
      {dialog?.kind === 'new-case' ? <NewCaseDialog /> : null}
      {dialog?.kind === 'new-project' ? <NewProjectDialog /> : null}
      {dialog?.kind === 'open-project' ? <OpenProjectDialog /> : null}
      {dialog?.kind === 'locate-project' ? <OpenProjectDialog locate={{ path: dialog.path, name: dialog.name }} /> : null}
      {dialog?.kind === 'passphrase' ? <PassphraseDialog path={dialog.path} /> : null}
      {dialog?.kind === 'import-qdpx' ? <QdpxImportDialog /> : null}
      {dialog?.kind === 'import-qdc' ? <QdcImportDialog /> : null}
      {dialog?.kind === 'link-passage' ? <LinkPassageDialog codingId={dialog.codingId} /> : null}
      {dialog?.kind === 'working-set' ? <WorkingSetDialog /> : null}
      {dialog?.kind === 'new-theme' ? <NewThemeDialog codeIds={dialog.codeIds} /> : null}
      {dialog?.kind === 'sample-projects' ? <SampleProjectsDialog /> : null}
      {dialog?.kind === 'add-language' ? <AddLanguageDialog /> : null}
      {dialog?.kind === 'translator-file' ? <TranslatorFileDialog id={dialog.id} /> : null}
      {dialog?.kind === 'palette' ? <CommandPalette mode={dialog.mode} /> : null}
      <CodeDialogs dialog={dialog} />
    </Fragment>
  );
}
