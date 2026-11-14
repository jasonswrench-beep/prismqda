import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Radio, RadioGroup } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { num, t, tn } from '../i18n';
import { fmt } from '../lib/statistics';
import { openAt, refreshEverything } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import type {
  DesignKind,
  DesignOutcome,
  DesignPlan,
  DesignPriority,
  IntegrationEntry,
  IntegrationHow,
  IntegrationInput,
  IntegrationStage,
  TriangulationInput,
  TriangulationMark,
  TriangulationRow,
} from '../types';
import { Notice } from './common';
import { codeChoices, EvidenceList } from './explore/shared';
import { useCaseVariables } from './explore/Matrix';
import { Described } from './JointMore';
import { useKept } from './jointShared';

/**
 * The parts of a mixed methods study that the researcher decides and the project keeps (spec 14.2 and 14.3): the design and what its
 * template sets up, the triangulation matrix with the researcher's marks, and the integration log. What the researcher writes is never
 * written for them; the words and numbers beside a topic are worked out from the project each time.
 */

const DESIGN_KINDS: DesignKind[] = ['convergent', 'explanatory_sequential', 'exploratory_sequential', 'embedded'];

type DisplayId = 'ordered' | 'typology' | 'side' | 'deviation' | 'similarity' | 'bubble';

interface DesignInfo {
  name: () => string;
  summary: () => string;
  steps: () => string[];
  /** The joint displays that suit the design. */
  displays: DisplayId[];
}

const DISPLAY_NAMES: Record<DisplayId, () => string> = {
  ordered: () => t('Case-ordered display'),
  typology: () => t('Typology table'),
  side: () => t('Side-by-side display'),
  deviation: () => t('Deviation table'),
  similarity: () => t('Similarity analysis'),
  bubble: () => t('Code by variable bubble plot'),
};

const DESIGNS: Record<DesignKind, DesignInfo> = {
  convergent: {
    name: () => t('Convergent'),
    summary: () => t('The numbers and the words are collected at about the same time, analysed separately, and then merged to see where they agree and where they do not.'),
    steps: () => [
      t('Analyse the numbers on the Statistics page and the coding in Explore.'),
      t('Set the findings of the two strands beside each other in the triangulation matrix, topic by topic.'),
      t('Mark each topic as convergence, complementarity, or discordance, and say why.'),
    ],
    displays: ['side', 'typology', 'deviation'],
  },
  explanatory_sequential: {
    name: () => t('Explanatory sequential'),
    summary: () => t('The numbers come first. The words then explain the results, for example why some cases scored high and others low.'),
    steps: () => [
      t('Analyse the numbers first. Put the group each case fell in (high, middle, low) in the variable this template makes, with Recode or Cut in the Data editor.'),
      t('Collect and code the material from the cases that need explaining, and put the explanations under the theme this template makes.'),
      t('Order the cases by the result and read what each said, and compare the groups with the deviation table.'),
    ],
    displays: ['ordered', 'deviation', 'typology'],
  },
  exploratory_sequential: {
    name: () => t('Exploratory sequential'),
    summary: () => t('The words come first. What they find is used to build the numbers, such as the items of a questionnaire.'),
    steps: () => [
      t('Code the interviews, and put the themes that should become items under the theme this template makes.'),
      t('Once the items are answered by a larger group, turn the themes into variables with Codes into variables.'),
      t('Use the deviation table and the bubble plot to see how the themes are spread across groups.'),
    ],
    displays: ['deviation', 'bubble', 'side'],
  },
  embedded: {
    name: () => t('Embedded'),
    summary: () => t('One strand is carried out within the other, for example interviews during a trial. The embedded strand supports the main one.'),
    steps: () => [
      t('Put the group each case is in (for example intervention or comparison) in the variable this template makes.'),
      t('Code the material of the embedded strand, and put what it found under the theme this template makes.'),
      t('Use the side-by-side display to put what the embedded strand says beside the numbers of the main strand.'),
    ],
    displays: ['side', 'typology'],
  },
};

const PRIORITIES: { id: DesignPriority; label: () => string }[] = [
  { id: 'equal', label: () => t('Equal priority') },
  { id: 'quantitative', label: () => t('The numbers have priority') },
  { id: 'qualitative', label: () => t('The words have priority') },
];

const STAGES: { id: IntegrationStage; label: () => string }[] = [
  { id: 'design', label: () => t('Design') },
  { id: 'data_collection', label: () => t('Data collection') },
  { id: 'analysis', label: () => t('Analysis') },
  { id: 'interpretation', label: () => t('Interpretation') },
  { id: 'reporting', label: () => t('Reporting') },
];

const HOWS: { id: IntegrationHow; label: () => string; help: () => string }[] = [
  { id: 'connecting', label: () => t('Connecting'), help: () => t('one strand leads to the other') },
  { id: 'building', label: () => t('Building'), help: () => t('one strand is used to build the other') },
  { id: 'merging', label: () => t('Merging'), help: () => t('the two are brought together') },
  { id: 'embedding', label: () => t('Embedding'), help: () => t('one sits within the other') },
  { id: 'transforming', label: () => t('Transforming'), help: () => t('one kind of data is turned into the other') },
];

const MARKS: { id: TriangulationMark; label: () => string; help: () => string }[] = [
  { id: '', label: () => t('Not marked'), help: () => t('not decided yet') },
  { id: 'convergence', label: () => t('Convergence'), help: () => t('the two strands say the same') },
  { id: 'complementarity', label: () => t('Complementarity'), help: () => t('they say different things that fit together') },
  { id: 'discordance', label: () => t('Discordance'), help: () => t('they conflict') },
];

const label = <T extends string>(list: { id: T; label: () => string }[], id: T): string => list.find((x) => x.id === id)?.label() ?? id;

/** Reads one of the kept lists, and again whenever the project changes or the caller says so. */
function useLoaded<T>(load: () => Promise<T>) {
  const version = useApp((s) => s.dataVersion);
  const codings = useApp((s) => s.sources.reduce((n, x) => n + x.codings, 0));
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [round, setRound] = useState(0);
  useEffect(() => {
    let live = true;
    load()
      .then((v) => {
        if (live) {
          setValue(v);
          setError(null);
        }
      })
      .catch((e) => live && setError(toApiError(e).message));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [version, codings, round]);
  const reload = useCallback(() => setRound((r) => r + 1), []);
  return { value, error, reload };
}

const PARTS: { id: 'design' | 'triangulation' | 'log'; name: () => string }[] = [
  { id: 'design', name: () => t('Design and templates') },
  { id: 'triangulation', name: () => t('Triangulation matrix') },
  { id: 'log', name: () => t('Integration log') },
];

export function DesignAndIntegration(props: { onOpenDisplay: (id: DisplayId) => void }) {
  const [part, setPart] = useKept<'design' | 'triangulation' | 'log'>('design.part', 'design');
  return (
    <div className="jd-page" data-testid="mixed-design-page">
      <p className="help">
        {t('What you decide about how the two kinds of data fit together is kept in the project: the design of the study, your marks on the triangulation matrix, and a log of where and how you brought them together. They are in your own words; the program never writes them for you, and the words and numbers beside a topic are worked out again each time.')}
      </p>
      <div className="field jd-which">
        <label htmlFor="md-part">{t('Which part')}</label>
        <select id="md-part" value={part} onChange={(e) => setPart(e.target.value as typeof part)} data-testid="md-part">
          {PARTS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name()}
            </option>
          ))}
        </select>
      </div>
      {part === 'triangulation' ? <Triangulation /> : part === 'log' ? <IntegrationLog /> : <Design onOpenDisplay={props.onOpenDisplay} />}
    </div>
  );
}

// ---------------------------------------------------------------- the design and its templates

function Design(props: { onOpenDisplay: (id: DisplayId) => void }) {
  const { value: design, error, reload } = useLoaded(() => call('mixed.design'));
  const [kind, setKind] = useKept<DesignKind>('design.kind', 'convergent');
  const [priority, setPriority] = useKept<DesignPriority | ''>('design.priority', '');
  const [purpose, setPurpose] = useKept('design.purpose', '');
  const [plan, setPlan] = useState<DesignPlan | null>(null);
  const [outcome, setOutcome] = useState<DesignOutcome | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // When there is a design already, the form starts from it.
  const [seeded, setSeeded] = useState(false);
  useEffect(() => {
    if (design && !seeded) {
      setSeeded(true);
      setKind(design.kind);
      setPriority(design.priority);
      setPurpose(design.purpose);
    }
  }, [design, seeded, setKind, setPriority, setPurpose]);

  useEffect(() => {
    let live = true;
    call('mixed.design.plan', { kind })
      .then((p) => live && setPlan(p))
      .catch(() => live && setPlan(null));
    return () => {
      live = false;
    };
  }, [kind, outcome]);

  const info = DESIGNS[kind];
  const chosenPriority = priority === '' ? (plan?.priority ?? 'equal') : priority;
  const changing = design !== null && design.kind !== kind;
  const apply = async () => {
    setBusy(true);
    setProblem(null);
    try {
      const log = design && design.kind === kind ? '' : t('Chose the {design} design.', { design: info.name().toLowerCase() });
      const result = await call('mixed.design.apply', { kind, priority: chosenPriority, purpose, log });
      setOutcome(result);
      reload();
      await refreshEverything();
      announce(t('Chose the {design} design.', { design: info.name().toLowerCase() }));
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const forget = async () => {
    setBusy(true);
    setProblem(null);
    try {
      await call('mixed.design.clear');
      setOutcome(null);
      setSeeded(false);
      reload();
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="ac-card mixed-card" aria-labelledby="h-md-design" data-testid="md-design">
      <h3 id="h-md-design" className="bk-h3">{t('Design and templates')}</h3>
      <p className="muted">{t('Choose the design of the study. A template makes the few variables and theme codes that design needs, shows the joint displays that suit it, and writes the first line of the integration log. Nothing is moved or changed in what is already in the project.')}</p>
      {design ? (
        <p role="status" data-testid="md-current">
          {t('The study follows the {design} design. {priority}.', { design: DESIGNS[design.kind].name().toLowerCase(), priority: label(PRIORITIES, design.priority) })}
        </p>
      ) : (
        <p className="muted" data-testid="md-none">{t('No design has been chosen yet.')}</p>
      )}
      {error ? <Notice>{error}</Notice> : null}
      <RadioGroup value={kind} onChange={(v) => { setKind(v as DesignKind); setPriority(''); setOutcome(null); }} className="field ac-radios" aria-label={t('The design')} data-testid="md-kinds">
        {DESIGN_KINDS.map((k) => (
          <Radio key={k} value={k} data-testid={`md-kind-${k}`}>
            <span className="radio-title">{DESIGNS[k].name()}</span>
            <span className="radio-help">{DESIGNS[k].summary()}</span>
          </Radio>
        ))}
      </RadioGroup>
      <div className="field">
        <label htmlFor="md-priority">{t('Which strand has priority')}</label>
        <select id="md-priority" value={chosenPriority} onChange={(e) => setPriority(e.target.value as DesignPriority)} data-testid="md-priority">
          {PRIORITIES.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label()}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="md-purpose">{t('Why the two kinds of data are mixed, in your words')}</label>
        <textarea id="md-purpose" rows={3} value={purpose} onChange={(e) => setPurpose(e.target.value)} data-testid="md-purpose" />
      </div>
      <div data-testid="md-plan">
        <h4 className="stats-h5">{t('What this design usually involves')}</h4>
        <ol>
          {info.steps().map((s, i) => (
            <li key={i}>{s}</li>
          ))}
        </ol>
        {plan && (plan.variables.length > 0 || plan.codes.length > 0) ? (
          <>
            <h4 className="stats-h5">{t('What the template would make')}</h4>
            <ul>
              {plan.variables.map((v) => (
                <li key={`v-${v.name}`}>{v.exists ? t('The variable {name} is already there and is left as it is.', { name: v.name }) : t('A variable of cases named {name}, with categories as its level, with no values yet.', { name: v.name })}</li>
              ))}
              {plan.codes.map((c) => (
                <li key={`c-${c.name}`}>{c.exists ? t('The theme code {name} is already there and is left as it is.', { name: c.name }) : t('A theme code named {name} in the Codebook.', { name: c.name })}</li>
              ))}
            </ul>
          </>
        ) : (
          <p className="muted">{t('This design needs no variable or code of its own: the matrix and the joint displays do the work.')}</p>
        )}
      </div>
      {problem ? <Notice>{problem}</Notice> : null}
      <div className="actions">
        <Button isDisabled={busy} onPress={() => void apply()} data-testid="md-apply">
          {design === null ? t('Choose this design and set it up') : changing ? t('Change to this design and set it up') : t('Save and set up anything missing')}
        </Button>
        {design ? (
          <Button isDisabled={busy} onPress={() => void forget()} data-testid="md-forget">
            {t('Forget the design')}
          </Button>
        ) : null}
      </div>
      {outcome ? (
        <p role="status" data-testid="md-outcome">
          {t('Chose the {design} design.', { design: DESIGNS[outcome.design.kind].name().toLowerCase() })}{' '}
          {outcome.variables_made.length + outcome.codes_made.length > 0
            ? t('Made {things}.', { things: [...outcome.variables_made.map((n) => t('the variable {name}', { name: n })), ...outcome.codes_made.map((n) => t('the theme code {name}', { name: n }))].join(', ') })
            : t('Nothing new was needed.')}{' '}
          {outcome.entry_id !== null ? t('The first line of the integration log is written.') : ''}
        </p>
      ) : null}
      <h4 className="stats-h5">{t('Joint displays that suit this design')}</h4>
      <ul className="md-displays">
        {info.displays.map((d) => (
          <li key={d}>
            {DISPLAY_NAMES[d]()}{' '}
            <Button className="bk-link" onPress={() => props.onOpenDisplay(d)} aria-label={t('Open the {display}', { display: DISPLAY_NAMES[d]() })} data-testid={`md-open-${d}`}>
              {t('Open it')}
            </Button>
          </li>
        ))}
      </ul>
      <p className="muted">{t('The documents of the two strands are grouped by their folder in the Documents pane. A template does not move documents; put the interviews in one folder and the survey answers in another yourself.')}</p>
    </section>
  );
}

// ---------------------------------------------------------------- the triangulation matrix

interface Draft {
  id: number | null;
  topic: string;
  code_id: number | null;
  variable: string;
  qualitative: string;
  quantitative: string;
  mark: TriangulationMark;
  note: string;
}

const EMPTY: Draft = { id: null, topic: '', code_id: null, variable: '', qualitative: '', quantitative: '', mark: '', note: '' };

function Triangulation() {
  const codes = useApp((s) => s.codes);
  const vars = useCaseVariables();
  const choices = useMemo(() => codeChoices(codes), [codes]);
  const { value: rows, error, reload } = useLoaded(() => call('mixed.triangulation'));
  const [draft, setDraft] = useState<Draft | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<number | null>(null);
  const [evidence, setEvidence] = useState<{ title: string; ids: number[] } | null>(null);
  const [busy, setBusy] = useState(false);

  const counts = useMemo(() => {
    const c = { convergence: 0, complementarity: 0, discordance: 0, none: 0 };
    for (const r of rows ?? []) c[r.mark === '' ? 'none' : r.mark] += 1;
    return c;
  }, [rows]);

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    setProblem(null);
    const input: TriangulationInput = { topic: draft.topic, code_id: draft.code_id, variable: draft.variable, qualitative: draft.qualitative, quantitative: draft.quantitative, mark: draft.mark, note: draft.note };
    try {
      if (draft.id === null) await call('mixed.triangulation.add', input);
      else await call('mixed.triangulation.update', { ...input, id: draft.id });
      announce(t('Saved the topic {topic}.', { topic: draft.topic }));
      setDraft(null);
      reload();
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const act = async (run: () => Promise<unknown>) => {
    setBusy(true);
    setProblem(null);
    try {
      await run();
      reload();
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const edit = (r: TriangulationRow) =>
    setDraft({ id: r.id, topic: r.topic, code_id: r.code_id, variable: r.variable, qualitative: r.qualitative, quantitative: r.quantitative, mark: r.mark, note: r.note });
  const set = (patch: Partial<Draft>) => setDraft((d) => (d ? { ...d, ...patch } : d));

  return (
    <section className="ac-card mixed-card" aria-labelledby="h-md-tri" data-testid="md-triangulation">
      <h3 id="h-md-tri" className="bk-h3">{t('Triangulation matrix')}</h3>
      <p className="muted">{t('A row for each topic: what the words say, what the numbers say, and your mark on how they relate. Link a topic to a code and a variable and the words and numbers behind it are shown beside it.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {problem ? <Notice>{problem}</Notice> : null}
      {rows && rows.length > 0 ? (
        <p role="status" data-testid="md-tri-summary">
          {tn(rows.length, { one: '{count} topic.', other: '{count} topics.' })}{' '}
          {t('{a} marked convergence, {b} complementarity, {c} discordance, and {d} not marked.', { a: num(counts.convergence), b: num(counts.complementarity), c: num(counts.discordance), d: num(counts.none) })}
        </p>
      ) : (
        <p className="muted" data-testid="md-tri-empty">{t('There are no topics yet.')}</p>
      )}
      {rows && rows.length > 0 ? (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The triangulation matrix')}>
          <table className="data jd-table">
            <caption className="visually-hidden">{t('Topics, what each strand says, what is in the data, and your mark')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Topic')}</th>
                <th scope="col">{t('What the words say')}</th>
                <th scope="col">{t('What the numbers say')}</th>
                <th scope="col">{t('In the data')}</th>
                <th scope="col">{t('Your mark')}</th>
                <th scope="col">{t('Actions')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={r.id} data-testid="md-tri-row">
                  <th scope="row">{r.topic}</th>
                  <td className="jd-cell">{r.qualitative === '' ? <span className="muted">{'—'}</span> : r.qualitative}</td>
                  <td className="jd-cell">{r.quantitative === '' ? <span className="muted">{'—'}</span> : r.quantitative}</td>
                  <td className="jd-cell">
                    <Evidence row={r} onEvidence={setEvidence} />
                  </td>
                  <td className="jd-cell">
                    <p className="jd-count">{label(MARKS, r.mark)}</p>
                    {r.note !== '' ? <p>{r.note}</p> : null}
                  </td>
                  <td>
                    <div className="md-actions">
                      <Button className="bk-link" onPress={() => edit(r)} aria-label={t('Edit the topic {topic}', { topic: r.topic })} data-testid="md-tri-edit">{t('Edit')}</Button>
                      <Button className="bk-link" isDisabled={busy || i === 0} onPress={() => void act(() => call('mixed.triangulation.move', { id: r.id, up: true }))} aria-label={t('Move {topic} up', { topic: r.topic })}>{t('Up')}</Button>
                      <Button className="bk-link" isDisabled={busy || i === rows.length - 1} onPress={() => void act(() => call('mixed.triangulation.move', { id: r.id, up: false }))} aria-label={t('Move {topic} down', { topic: r.topic })}>{t('Down')}</Button>
                      {confirm === r.id ? (
                        <span role="group" aria-label={t('Remove {topic}?', { topic: r.topic })}>
                          <Button className="bk-link" onPress={() => void act(async () => { await call('mixed.triangulation.remove', { id: r.id }); setConfirm(null); })} data-testid="md-tri-remove-yes">{t('Remove it for good')}</Button>{' '}
                          <Button className="bk-link" onPress={() => setConfirm(null)}>{t('Keep it')}</Button>
                        </span>
                      ) : (
                        <Button className="bk-link" onPress={() => setConfirm(r.id)} aria-label={t('Remove the topic {topic}', { topic: r.topic })} data-testid="md-tri-remove">{t('Remove')}</Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {evidence ? <EvidenceList title={evidence.title} codingIds={evidence.ids} onClose={() => setEvidence(null)} testId="md-tri-evidence" /> : null}
      {draft === null ? (
        <div className="actions">
          <Button onPress={() => setDraft({ ...EMPTY })} data-testid="md-tri-add">{t('Add a topic')}</Button>
        </div>
      ) : (
        <form
          className="md-form"
          aria-labelledby="h-md-tri-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
          data-testid="md-tri-form"
        >
          <h4 id="h-md-tri-form" className="bk-h3">{draft.id === null ? t('A new topic') : t('Editing the topic {topic}', { topic: rows?.find((r) => r.id === draft.id)?.topic ?? '' })}</h4>
          <div className="field">
            <label htmlFor="md-tri-topic">{t('Topic')}</label>
            <input id="md-tri-topic" type="text" value={draft.topic} onChange={(e) => set({ topic: e.target.value })} autoComplete="off" data-testid="md-tri-topic" />
          </div>
          <div className="field">
            <label htmlFor="md-tri-code">{t('Code whose words are the evidence (optional)')}</label>
            <select id="md-tri-code" value={draft.code_id ?? ''} onChange={(e) => set({ code_id: e.target.value === '' ? null : Number(e.target.value) })} data-testid="md-tri-code">
              <option value="">{t('No code')}</option>
              {choices.map((c) => (
                <option key={c.id} value={c.id}>
                  {`${' '.repeat(c.depth ?? 0)}${c.label}`}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="md-tri-variable">{t('Variable whose numbers are the evidence (optional)')}</label>
            <select id="md-tri-variable" value={draft.variable} onChange={(e) => set({ variable: e.target.value })} data-testid="md-tri-variable">
              <option value="">{t('No variable')}</option>
              {vars
                .filter((v) => v.level !== 'text')
                .map((v) => (
                  <option key={v.name} value={v.name}>
                    {v.name}
                  </option>
                ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="md-tri-words">{t('What the words say, in your words')}</label>
            <textarea id="md-tri-words" rows={3} value={draft.qualitative} onChange={(e) => set({ qualitative: e.target.value })} data-testid="md-tri-words" />
          </div>
          <div className="field">
            <label htmlFor="md-tri-numbers">{t('What the numbers say, in your words')}</label>
            <textarea id="md-tri-numbers" rows={3} value={draft.quantitative} onChange={(e) => set({ quantitative: e.target.value })} data-testid="md-tri-numbers" />
          </div>
          <RadioGroup value={draft.mark} onChange={(v) => set({ mark: v as TriangulationMark })} className="field ac-radios" aria-label={t('Your mark')} data-testid="md-tri-mark">
            {MARKS.map((m) => (
              <Radio key={m.id || 'none'} value={m.id} data-testid={`md-tri-mark-${m.id || 'none'}`}>
                <span className="radio-title">{m.label()}</span>
                <span className="radio-help">{m.help()}</span>
              </Radio>
            ))}
          </RadioGroup>
          <div className="field">
            <label htmlFor="md-tri-note">{t('Why you marked it so')}</label>
            <textarea id="md-tri-note" rows={2} value={draft.note} onChange={(e) => set({ note: e.target.value })} data-testid="md-tri-note" />
          </div>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={busy} data-testid="md-tri-save">{t('Save the topic')}</Button>
            <Button onPress={() => { setDraft(null); setProblem(null); }} data-testid="md-tri-cancel">{t('Cancel')}</Button>
          </div>
        </form>
      )}
    </section>
  );
}

function Evidence(props: { row: TriangulationRow; onEvidence: (e: { title: string; ids: number[] }) => void }) {
  const { row } = props;
  const e = row.evidence;
  if (!e) {
    return (
      <>
        <span className="muted">{row.problem ?? t('Not linked to a code or a variable.')}</span>
      </>
    );
  }
  return (
    <>
      {e.words ? (
        <div data-testid="md-tri-words-evidence">
          <p className="jd-count">{t('{code}: {excerpts} from {cases}', { code: row.code_name ?? '', excerpts: tn(e.words.excerpts, { one: '{count} excerpt', other: '{count} excerpts' }), cases: tn(e.words.cases, { one: '{count} case', other: '{count} cases' }) })}</p>
          <ul className="jd-quotes">
            {e.words.quotes.map((q) => (
              <li key={q.coding_id}>
                <q>{q.text ?? t('An area of a picture')}</q> <span className="muted">({q.case_name})</span>{' '}
                <Button className="bk-link" onPress={() => void openAt(q.source_id, q.start, q.end, q.coding_id)} aria-label={t('Open {source} at these words, from {case}', { source: q.source, case: q.case_name })}>
                  {t('Open')}
                </Button>
              </li>
            ))}
          </ul>
          {e.words.more > 0 ? (
            <Button className="bk-link" onPress={() => props.onEvidence({ title: row.topic, ids: e.words!.coding_ids })} data-testid="md-tri-more">
              {t('Show all, {count} more', { count: num(e.words.more) })}
            </Button>
          ) : null}
        </div>
      ) : null}
      {e.numbers ? (
        <div data-testid="md-tri-numbers-evidence">
          <p className="jd-count">{e.numbers.column.name}</p>
          <p className="muted">{e.numbers.whole ? t('All the cases that hold text') : t('Cases that have the code')}</p>
          <Described cell={e.numbers.with} column={e.numbers.column} />
          {e.numbers.without ? (
            <>
              <p className="muted">{t('The other cases')}</p>
              <Described cell={e.numbers.without} column={e.numbers.column} />
            </>
          ) : null}
          {e.numbers.difference !== null ? <p className="jd-diff">{t('Difference of the means: {difference}', { difference: `${e.numbers.difference > 0 ? '+' : ''}${fmt(e.numbers.difference)}` })}</p> : null}
        </div>
      ) : null}
      {row.problem ? <p className="muted">{row.problem}</p> : null}
    </>
  );
}

// ---------------------------------------------------------------- the integration log

interface EntryDraft {
  id: number | null;
  on_date: string;
  stage: IntegrationStage;
  how: IntegrationHow;
  what: string;
  why: string;
}

const NEW_ENTRY: EntryDraft = { id: null, on_date: '', stage: 'analysis', how: 'merging', what: '', why: '' };

function IntegrationLog() {
  const { value: entries, error, reload } = useLoaded(() => call('mixed.integration'));
  const [draft, setDraft] = useState<EntryDraft | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<EntryDraft>) => setDraft((d) => (d ? { ...d, ...patch } : d));

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    setProblem(null);
    const input: IntegrationInput = { on_date: draft.on_date.trim() === '' ? null : draft.on_date.trim(), stage: draft.stage, how: draft.how, what: draft.what, why: draft.why };
    try {
      if (draft.id === null) await call('mixed.integration.add', input);
      else await call('mixed.integration.update', { ...input, id: draft.id });
      announce(t('Saved the entry.'));
      setDraft(null);
      reload();
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  const remove = async (id: number) => {
    setBusy(true);
    setProblem(null);
    try {
      await call('mixed.integration.remove', { id });
      setConfirm(null);
      reload();
    } catch (e) {
      setProblem(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="ac-card mixed-card" aria-labelledby="h-md-log" data-testid="md-log">
      <h3 id="h-md-log" className="bk-h3">{t('Integration log')}</h3>
      <p className="muted">{t('A dated record of where and how you brought the numbers and the words together, and why. It goes into the report, so a reader can see how the two were mixed.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {problem ? <Notice>{problem}</Notice> : null}
      {entries && entries.length > 0 ? (
        <div className="table-wrap" tabIndex={0} role="region" aria-label={t('The integration log')}>
          <table className="data jd-table">
            <caption className="visually-hidden">{t('Entries of the integration log, the oldest first')}</caption>
            <thead>
              <tr>
                <th scope="col">{t('Date')}</th>
                <th scope="col">{t('Stage')}</th>
                <th scope="col">{t('How')}</th>
                <th scope="col">{t('What was done')}</th>
                <th scope="col">{t('Why')}</th>
                <th scope="col">{t('Actions')}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e: IntegrationEntry) => (
                <tr key={e.id} data-testid="md-log-row">
                  <th scope="row">{e.on_date}</th>
                  <td>{label(STAGES, e.stage)}</td>
                  <td>{label(HOWS, e.how)}</td>
                  <td className="jd-cell">{e.what}</td>
                  <td className="jd-cell">{e.why === '' ? <span className="muted">{'—'}</span> : e.why}</td>
                  <td>
                    <div className="md-actions">
                      <Button className="bk-link" onPress={() => setDraft({ id: e.id, on_date: e.on_date, stage: e.stage, how: e.how, what: e.what, why: e.why })} aria-label={t('Edit the entry of {date}', { date: e.on_date })} data-testid="md-log-edit">{t('Edit')}</Button>
                      {confirm === e.id ? (
                        <span role="group" aria-label={t('Remove the entry of {date}?', { date: e.on_date })}>
                          <Button className="bk-link" onPress={() => void remove(e.id)} data-testid="md-log-remove-yes">{t('Remove it for good')}</Button>{' '}
                          <Button className="bk-link" onPress={() => setConfirm(null)}>{t('Keep it')}</Button>
                        </span>
                      ) : (
                        <Button className="bk-link" onPress={() => setConfirm(e.id)} aria-label={t('Remove the entry of {date}', { date: e.on_date })} data-testid="md-log-remove">{t('Remove')}</Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="muted" data-testid="md-log-empty">{t('The log is empty.')}</p>
      )}
      {draft === null ? (
        <div className="actions">
          <Button onPress={() => setDraft({ ...NEW_ENTRY })} data-testid="md-log-add">{t('Add an entry')}</Button>
        </div>
      ) : (
        <form
          className="md-form"
          aria-labelledby="h-md-log-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
          data-testid="md-log-form"
        >
          <h4 id="h-md-log-form" className="bk-h3">{draft.id === null ? t('A new entry') : t('Editing the entry')}</h4>
          <div className="field">
            <label htmlFor="md-log-date">{t('Date, as year-month-day (today when left empty)')}</label>
            <input id="md-log-date" type="text" value={draft.on_date} onChange={(e) => set({ on_date: e.target.value })} placeholder="2026-11-08" autoComplete="off" data-testid="md-log-date" />
          </div>
          <div className="field">
            <label htmlFor="md-log-stage">{t('Stage of the study')}</label>
            <select id="md-log-stage" value={draft.stage} onChange={(e) => set({ stage: e.target.value as IntegrationStage })} data-testid="md-log-stage">
              {STAGES.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label()}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="md-log-how">{t('How the two were brought together')}</label>
            <select id="md-log-how" value={draft.how} onChange={(e) => set({ how: e.target.value as IntegrationHow })} data-testid="md-log-how">
              {HOWS.map((h) => (
                <option key={h.id} value={h.id}>
                  {`${h.label()}: ${h.help()}`}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="md-log-what">{t('What was done')}</label>
            <textarea id="md-log-what" rows={3} value={draft.what} onChange={(e) => set({ what: e.target.value })} data-testid="md-log-what" />
          </div>
          <div className="field">
            <label htmlFor="md-log-why">{t('Why')}</label>
            <textarea id="md-log-why" rows={2} value={draft.why} onChange={(e) => set({ why: e.target.value })} data-testid="md-log-why" />
          </div>
          <div className="actions">
            <Button type="submit" className="primary" isDisabled={busy} data-testid="md-log-save">{t('Save the entry')}</Button>
            <Button onPress={() => { setDraft(null); setProblem(null); }} data-testid="md-log-cancel">{t('Cancel')}</Button>
          </div>
        </form>
      )}
    </section>
  );
}

// ---------------------------------------------------------------- recording a step in the log

/** A button that writes what was just done in Mixed Methods into the integration log, for the person to edit later. */
export function RecordInLog(props: { what: string; how?: IntegrationHow; testId?: string }) {
  const [done, setDone] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    setDone(false);
    setProblem(null);
  }, [props.what]);
  const record = async () => {
    try {
      await call('mixed.integration.add', { stage: 'analysis', how: props.how ?? 'transforming', what: props.what, why: '' });
      setDone(true);
      announce(t('Recorded in the integration log.'));
    } catch (e) {
      setProblem(toApiError(e).message);
    }
  };
  return (
    <span className="md-record">
      <Button className="link-button" isDisabled={done} onPress={() => void record()} data-testid={props.testId ?? 'mixed-record'}>
        {done ? t('Recorded in the integration log') : t('Record this in the integration log')}
      </Button>
      {problem ? <Notice>{problem}</Notice> : null}
    </span>
  );
}

export type { DisplayId };
