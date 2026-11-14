import { Button, Tab, TabList, TabPanel, Tabs } from 'react-aria-components';
import { goTo } from '../state/actions';
import { appStore, useApp, type ExploreTool } from '../state/app';
import { ConceptMapTool } from './explore/ConceptMap';
import { Crosstabs } from './explore/Crosstabs';
import { CodeMatrix } from './explore/Matrix';
import { QueryBuilder } from './explore/Query';
import { DocumentPortrait } from './explore/Portrait';
import { CodeRelations } from './explore/Relations';
import { CodeSummary } from './explore/Summary';
import { Visualizations } from './explore/Visualizations';
import { t } from '../i18n';

const TOOLS: { id: ExploreTool; label: string }[] = [
  { id: 'query', get label() { return t('Query Builder'); } },
  { id: 'matrix', get label() { return t('Code Matrix'); } },
  { id: 'crosstab', get label() { return t('Crosstabs'); } },
  { id: 'relations', get label() { return t('Code Relations'); } },
  { id: 'summary', get label() { return t('Code Summary'); } },
  { id: 'visualize', get label() { return t('Visualizations'); } },
  { id: 'map', get label() { return t('Concept Map'); } },
  { id: 'portrait', get label() { return t('Document Portrait'); } },
];

/**
 * Explore: the tools that ask questions of the coding. They share the project, so a query's codes can be carried to the matrix,
 * and what was set up in one is still there when another is used. Every number can be opened to the passages behind it.
 */
export function ExploreView() {
  const tool = useApp((s) => s.exploreTool);
  return (
    <main id="main-content" tabIndex={-1} className="view-page ex-explore-page" aria-labelledby="h-explore">
      <h2 id="h-explore">{t('Explore')}</h2>
      <Tabs selectedKey={tool} onSelectionChange={(k) => appStore.set({ exploreTool: k as ExploreTool })} className="bk-editor-tabs">
        <TabList aria-label={t('Explore tools')} className="bk-editor-tablist">
          {TOOLS.map((t) => (
            <Tab key={t.id} id={t.id} className="bk-tab">
              {t.label}
            </Tab>
          ))}
        </TabList>
        <TabPanel id="query" className="bk-editor-panel">
          <QueryBuilder />
        </TabPanel>
        <TabPanel id="matrix" className="bk-editor-panel">
          <CodeMatrix />
        </TabPanel>
        <TabPanel id="crosstab" className="bk-editor-panel">
          <Crosstabs />
        </TabPanel>
        <TabPanel id="relations" className="bk-editor-panel">
          <CodeRelations />
        </TabPanel>
        <TabPanel id="summary" className="bk-editor-panel">
          <CodeSummary />
        </TabPanel>
        <TabPanel id="visualize" className="bk-editor-panel">
          <Visualizations />
        </TabPanel>
        <TabPanel id="map" className="bk-editor-panel">
          <ConceptMapTool />
        </TabPanel>
        <TabPanel id="portrait" className="bk-editor-panel">
          <DocumentPortrait />
        </TabPanel>
      </Tabs>
      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}
