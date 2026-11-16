import { Button } from 'react-aria-components';
import { goTo } from '../state/actions';
import { useApp } from '../state/app';
import { AiView } from './AiView';
import { CoachBar } from './CoachBar';
import { TeamView } from './TeamView';
import { StatisticsView } from './StatisticsView';
import { VariablesView } from './VariablesView';
import { MixedMethodsView } from './MixedMethodsView';
import { TextAnalyticsView } from './TextAnalyticsView';
import { AutoCodeView } from './AutoCodeView';
import { DataView } from './DataView';
import { HomeView } from './HomeView';
import { Icon } from './Icon';
import { MemosArea } from './MemosPanel';
import { PassageLinksPage } from './PassageLinks';
import { ThemesView } from './ThemesView';
import { WorkingSetBar } from './WorkingSet';
import { RibbonArea } from './Ribbon';
import { PrivacyView } from './PrivacyView';
import { ReportView } from './ReportView';
import { CodebookEditor } from './CodebookEditor';
import { ExploreView } from './ExploreView';
import { ImportPage } from './ImportPage';
import { SettingsView } from './SettingsView';
import { TitleBar } from './TitleBar';
import { Workspace as CodeWorkspace } from './Workspace';
import { t } from '../i18n';

/**
 * The frame every screen sits in: the title bar, the ribbon tabs and their commands, and the work area (the coding
 * workspace, or one of the pages that open in its place). Only the work area changes when you open a page; the rest
 * stays where it is (screen specification book, Main Application Layout).
 */
export function Frame() {
  const project = useApp((s) => s.project);
  const workspace = useApp((s) => s.workspace);
  // An instructor's assignment can close pages of the program to the student and keep the coach on (decision 200).
  const assignment = project?.packet?.assignment ?? null;
  const closed = (assignment?.closed ?? []).includes(workspace);
  const shown = closed ? null : workspace;
  // A packet says whom it was made for, so the person who opens it knows whose coding they are making (decision 161).
  const packetLines = [
    ...(project?.packet ? [t('This project is a packet made for {coder}, from "{from}". Code the documents, then close it and send this file back; whoever sent it brings it in with Team > Merge.', { coder: project.packet.coder_name, from: project.packet.from_project })] : []),
    ...(project?.notices ?? []),
  ];

  return (
    <div className="app-tabs frame bk-frame">
      <TitleBar />
      <RibbonArea />

      {packetLines.length > 0 ? (
        <section className="notices-region" aria-label={t('Notices about this project')}>
          <ul className="notices">
            {packetLines.map((n, i) => (
              <li key={i}>{n}</li>
            ))}
          </ul>
        </section>
      ) : null}

      <WorkingSetBar />

      {assignment?.coach_locked && workspace === 'code' ? <CoachBar /> : null}

      {workspace !== 'code' ? (
        <nav className="bk-pagebar" aria-label={t('Page')}>
          <Button className="bk-back" onPress={() => goTo('code')}>
            <Icon name="ui-chevron-right" size={16} className="flip" />{t('Back to the coding workspace')}</Button>
        </nav>
      ) : null}

      <div className="tab-body" data-workspace={workspace}>
        {closed ? (
          <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-closed" data-testid="page-closed">
            <h2 id="h-closed">{t('This page is closed')}</h2>
            <p className="help">{t('Your instructor has closed this page for this assignment. It opens again when you work outside the assignment.')}</p>
          </main>
        ) : null}
        {shown === 'home' ? <HomeView /> : null}
        {shown === 'data' ? <DataView /> : null}
        {shown === 'code' ? <CodeWorkspace /> : null}
        {shown === 'codebook' ? <CodebookEditor /> : null}
        {shown === 'themes' ? <ThemesView /> : null}
        {shown === 'explore' ? <ExploreView /> : null}
        {shown === 'autocode' ? <AutoCodeView /> : null}
        {shown === 'ai' ? <AiView /> : null}
        {shown === 'team' ? <TeamView /> : null}
        {shown === 'text' ? <TextAnalyticsView /> : null}
        {shown === 'stats' ? <StatisticsView /> : null}
        {shown === 'variables' ? <VariablesView /> : null}
        {shown === 'mixed' ? <MixedMethodsView /> : null}
        {shown === 'import' ? <ImportPage /> : null}
        {shown === 'synthesize' ? (
          <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-memos">
            <h2 id="h-memos">{t('Memos')}</h2>
            <p className="help">{t('Write down what you notice and decide. Memos can be about a coding, a code, a source, or the whole project.')}</p>
            <MemosArea allowAll idPrefix="page-memo" />
            <PassageLinksPage />
          </main>
        ) : null}
        {shown === 'report' ? <ReportView /> : null}
        {shown === 'privacy' ? <PrivacyView /> : null}
        {shown === 'settings' ? <SettingsView /> : null}
      </div>
    </div>
  );
}
