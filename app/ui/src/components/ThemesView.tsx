import { useCallback, useEffect, useState } from 'react';
import { Button, Input, Label, TextArea, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { goExplore, goTo, openAt, refreshCodes, selectCode, setActiveCodes, showPassages } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { ThemeCard, ThemeCode, ThemeOverview, ThemeStage } from '../types';
import { Notice } from './common';
import { beginPointerDrag } from './pointerDrag';
import { ThemeAi } from './ThemeAi';
import { Tick } from './Tick';
import { num, t, tn } from '../i18n';

/** The attribute that marks a place a code can be dropped on: a theme's number, or `none` for the codes under no theme. */
const DROP = 'data-drop-theme';

const STAGES: { id: ThemeStage; label: () => string; help: () => string }[] = [
  { id: 'candidate', label: () => t('Candidate'), help: () => t('A grouping of codes that might hang together, not yet checked.') },
  { id: 'reviewed', label: () => t('Reviewed'), help: () => t('Checked against the coded passages and against the data as a whole.') },
  { id: 'defined', label: () => t('Defined'), help: () => t('Named and defined, ready to be written up.') },
];

/** A code in a list: its name, what it holds, and the ways to move it. */
function CodeRow(props: { code: ThemeCode; themes: ThemeCard[]; here: number | null; readOnly: boolean; onMove: (codeId: number, to: number | null) => void; ticked?: boolean; onTick?: (on: boolean) => void }) {
  const { code } = props;
  const allCodes = useApp((s) => s.codes);
  // A code under another code (a sub-code) stands under that code, which is not one of the themes in the list.
  const parentCode = props.here !== null && !props.themes.some((th) => th.id === props.here) ? allCodes.find((c) => c.id === props.here) : undefined;
  return (
    <li
      className="th-code"
      // Dragged with the pointer, not with the browser's drag and drop, which the Windows window does not pass to the page (components/pointerDrag.ts).
      // (in the capture phase: the tick inside a row stops a press from reaching it on the way up)
      onPointerDownCapture={props.readOnly ? undefined : (e) => beginPointerDrag(e, { label: code.name, target: DROP, onDrop: (where) => props.onMove(code.id, where === 'none' ? null : Number(where)) })}
      data-testid="theme-code"
    >
      {props.onTick ? (
        <Tick isSelected={props.ticked ?? false} onChange={props.onTick} testId="theme-code-tick">
          <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: code.color }} />
          {code.name}
        </Tick>
      ) : (
        <span className="th-code-name">
          <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: code.color }} />
          {code.name}
        </span>
      )}
      <span className="muted th-code-counts">{`${tn(code.passages, { one: '{count} passage', other: '{count} passages' })}, ${tn(code.documents, { one: '{count} document', other: '{count} documents' })}`}</span>
      <Button
        className="bk-link"
        aria-label={t('Show the passages coded {name}', { name: code.name })}
        onPress={() => {
          setActiveCodes([code.id]);
          showPassages();
        }}
      >{t('Passages')}</Button>
      {props.readOnly ? null : (
        <>
          <label className="visually-hidden" htmlFor={`move-${code.id}`}>{t('Move {name} to', { name: code.name })}</label>
          <select
            id={`move-${code.id}`}
            value={props.here === null ? '' : String(props.here)}
            onChange={(e) => props.onMove(code.id, e.target.value === '' ? null : Number(e.target.value))}
            data-testid="theme-move"
          >
            <option value="">{t('Under no theme')}</option>
            {parentCode ? <option value={parentCode.id}>{t('Under the code {name}', { name: parentCode.name })}</option> : null}
            {props.themes.map((th) => (
              <option key={th.id} value={th.id}>{th.name}</option>
            ))}
          </select>
        </>
      )}
    </li>
  );
}

function ThemeCardView(props: { card: ThemeCard; themes: ThemeCard[]; readOnly: boolean; onChanged: () => void; onMove: (codeId: number, to: number | null) => void }) {
  const { card } = props;
  const [name, setName] = useState(card.name);
  const [definition, setDefinition] = useState(card.definition);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setName(card.name);
    setDefinition(card.definition);
  }, [card.name, card.definition]);
  const changed = name.trim() !== card.name || definition !== card.definition;

  const save = async () => {
    setError(null);
    try {
      await call('code.update', { id: card.id, name, definition });
      announce(t('Saved the theme {name}.', { name: name.trim() }));
      props.onChanged();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };
  const setStage = async (stage: ThemeStage) => {
    setError(null);
    try {
      await call('code.update', { id: card.id, theme_stage: stage });
      announce(t('{name} is now {stage}.', { name: card.name, stage: STAGES.find((s) => s.id === stage)!.label().toLowerCase() }));
      props.onChanged();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  return (
    <section
      className="ac-card th-card"
      aria-labelledby={`th-${card.id}`}
      data-testid="theme-card"
      {...{ [DROP]: props.readOnly ? undefined : card.id }}
    >
      <div className="th-head">
        <h3 id={`th-${card.id}`} className="bk-h3">
          <span className="code-dot" aria-hidden="true" style={{ ['--swatch-color' as string]: card.color }} />
          {card.name}
        </h3>
        <label htmlFor={`stage-${card.id}`} className="visually-hidden">{t('Where {name} stands', { name: card.name })}</label>
        <select id={`stage-${card.id}`} value={card.stage} disabled={props.readOnly} onChange={(e) => void setStage(e.target.value as ThemeStage)} data-testid="theme-stage">
          {STAGES.map((s) => (
            <option key={s.id} value={s.id}>{s.label()}</option>
          ))}
        </select>
      </div>
      <p className="muted" data-testid="theme-counts">
        {`${tn(card.passages, { one: '{count} passage', other: '{count} passages' })} · ${tn(card.documents, { one: '{count} document', other: '{count} documents' })} · ${tn(card.cases, { one: '{count} case', other: '{count} cases' })}`}
        {card.memos > 0 ? ` · ${tn(card.memos, { one: '{count} memo', other: '{count} memos' })}` : ''}
      </p>
      {error ? <Notice>{error}</Notice> : null}
      {props.readOnly ? (
        <p>{card.definition.trim() === '' ? <span className="muted">{t('No definition yet.')}</span> : card.definition}</p>
      ) : (
        <>
          <TextField value={name} onChange={setName} className="field">
            <Label>{t('Name')}</Label>
            <Input data-testid="theme-name" />
          </TextField>
          <TextField value={definition} onChange={setDefinition} className="field">
            <Label>{t('Definition: what the theme says, in a sentence or two')}</Label>
            <TextArea rows={3} data-testid="theme-definition" />
          </TextField>
          <div className="actions">
            <Button onPress={() => void save()} isDisabled={!changed || name.trim() === ''} data-testid="theme-save">{t('Save name and definition')}</Button>
            <Button className="bk-link" onPress={() => { selectCode(card.id); goTo('codebook'); }}>{t('Open in the Codebook Editor')}</Button>
          </div>
        </>
      )}

      <h4 className="bk-h3">{t('Codes under this theme')}</h4>
      {card.codes.length === 0 ? (
        <p className="muted" data-testid="theme-no-codes">{t('No code is under this theme yet. Drag one here, or choose this theme in the list beside a code.')}</p>
      ) : (
        <ul className="th-codes">
          {card.codes.map((c) => (
            <CodeRow key={c.id} code={c} themes={props.themes} here={c.parent_id} readOnly={props.readOnly} onMove={props.onMove} />
          ))}
        </ul>
      )}

      {card.quotes.length > 0 ? (
        <>
          <h4 className="bk-h3">{t('Quotations that show it')}</h4>
          <ul className="th-quotes">
            {card.quotes.map((q) => (
              <li key={q.coding_id} data-testid="theme-quote">
                <blockquote className="bk-quote">{`“${q.text.replace(/\s+/g, ' ').trim()}”`}</blockquote>
                <p className="muted">
                  {`${q.source_name} · ${q.code_name}${q.example ? ' · ' + t('marked as an example') : ''} `}
                  <Button className="bk-link" aria-label={t('Open this passage in {name}', { name: q.source_name })} onPress={() => void openAt(q.source_id, q.start, q.end, q.coding_id)}>{t('Open')}</Button>
                </p>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </section>
  );
}

/** The Themes page: every theme with its codes, what they hold, and quotations, and the codes still to place (decision 167). */
export function ThemesView() {
  const readOnly = useApp((s) => s.project?.read_only ?? false);
  const version = useApp((s) => s.dataVersion);
  const codes = useApp((s) => s.codes);
  const [data, setData] = useState<ThemeOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);

  const load = useCallback(async () => {
    try {
      setData(await call('theme.overview', {}));
      setError(null);
    } catch (e) {
      setError(toApiError(e).message);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load, version, codes]);

  const changed = async () => {
    await refreshCodes();
    appStore.set((s) => ({ dataVersion: s.dataVersion + 1 }));
  };
  const move = async (codeId: number, to: number | null) => {
    // Already there: nothing to move (a drop on the theme the code is under would only re-sort it and add a step to Undo).
    if ((codes.find((c) => c.id === codeId)?.parent_id ?? null) === to) return;
    try {
      await call('code.move', { id: codeId, parent_id: to });
      const where = to === null ? t('under no theme') : t('under {name}', { name: data?.themes.find((th) => th.id === to)?.name ?? '' });
      announce(t('Moved {code} {where}.', { code: codes.find((c) => c.id === codeId)?.name ?? '', where }));
      await changed();
    } catch (e) {
      setError(toApiError(e).message);
    }
  };
  const makeMap = async () => {
    try {
      const r = await call('theme.map', {});
      appStore.set({ exploreMap: { mapId: r.map_id, selected: null } });
      goExplore('map');
    } catch (e) {
      setError(toApiError(e).message);
    }
  };

  const themes = data?.themes ?? [];
  return (
    <main id="main-content" tabIndex={-1} className="view-page" aria-labelledby="h-themes" data-testid="themes-view">
      <h2 id="h-themes">{t('Themes')}</h2>
      <p className="help">{t('A theme is a code that groups other codes. Here you group the codes into candidate themes, check each against its passages, and define and name it. Drag a code onto a theme, or choose a theme in the list beside it; the codebook shows the same grouping. A theme stands as a candidate until you mark it reviewed, and defined once it is named and defined.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      <div className="actions">
        {readOnly ? null : <Button className="primary" onPress={() => appStore.set({ dialog: { kind: 'new-theme', codeIds: [] } })} data-testid="theme-add">{t('Add a theme…')}</Button>}
        <Button onPress={() => void makeMap()} isDisabled={themes.length === 0} data-testid="theme-map">{t('Make a thematic map')}</Button>
      </div>

      {data === null && !error ? <p className="muted">{t('Looking at the themes…')}</p> : null}
      {data !== null && themes.length === 0 ? <p className="help" data-testid="themes-empty">{t('There is no theme yet. Add one, then move codes under it; or tick codes below and make a theme from them.')}</p> : null}
      {themes.length > 0 ? (
        <p className="muted" data-testid="themes-stages">
          {STAGES.map((s) => `${s.label()}: ${num(themes.filter((th) => th.stage === s.id).length)}`).join(' · ')}
        </p>
      ) : null}
      <div className="th-grid">
        {themes.map((card) => (
          <ThemeCardView key={card.id} card={card} themes={themes} readOnly={readOnly} onChanged={() => void changed()} onMove={(id, to) => void move(id, to)} />
        ))}
      </div>

      {data !== null ? (
        <section
          className="ac-card th-unplaced"
          aria-labelledby="h-unplaced"
          data-testid="themes-unassigned"
          {...{ [DROP]: readOnly ? undefined : 'none' }}
        >
          <h3 id="h-unplaced" className="bk-h3">{t('Codes under no theme')}</h3>
          {data.unassigned.length === 0 ? (
            <p className="muted" data-testid="themes-all-placed">{t('Every code is under a theme.')}</p>
          ) : (
            <>
              <p className="muted">{t('These codes are still to place. Tick some and make a theme from them, or move each under a theme.')}</p>
              <ul className="th-codes">
                {data.unassigned.map((c) => (
                  <CodeRow key={c.id} code={c} themes={themes} here={null} readOnly={readOnly} onMove={(id, to) => void move(id, to)} ticked={ticked.includes(c.id)} onTick={readOnly ? undefined : (on) => setTicked(on ? [...ticked, c.id] : ticked.filter((x) => x !== c.id))} />
                ))}
              </ul>
              {readOnly ? null : (
                <div className="actions">
                  <Button
                    onPress={() => {
                      appStore.set({ dialog: { kind: 'new-theme', codeIds: ticked.filter((id) => data.unassigned.some((c) => c.id === id)) } });
                      setTicked([]);
                    }}
                    isDisabled={ticked.filter((id) => data.unassigned.some((c) => c.id === id)).length === 0}
                    data-testid="theme-from-ticked"
                  >{t('Make a theme from the ticked codes…')}</Button>
                </div>
              )}
            </>
          )}
        </section>
      ) : null}

      <ThemeAi onChanged={() => void changed()} />

      <p className="bk-editor-foot">
        <Button className="bk-link" onPress={() => goTo('code')}>{t('Back to the coding workspace')}</Button>
      </p>
    </main>
  );
}
