import { useEffect, useState, type ReactNode } from 'react';
import { Button, Checkbox, Input, Label, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { cleanHistoryName, CONFIDENCE_LEVELS, KEEP_BACKUP_CHOICES, PAIR_METHOD_IDS, RESAMPLE_CHOICES, TEXT_SIZES, THEMES, type PairMethod, type StatisticsScope, type TextWidth, type Theme } from '../lib/settings';
import { levelText } from '../lib/statistics';
import { readJson, writeJson } from '../lib/storage';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { goTo, resetPanelSizes, resetSettings, setAiMode, setBackupFolder, updateSettings } from '../state/actions';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import { EncryptionPanel } from './EncryptionPanel';
import { Icon, type IconName } from './Icon';
import { num, rich, t } from '../i18n';
import { LanguageBody } from './LanguageBody';
import { UpdatesCard } from './UpdatesCard';

type SectionId = 'general' | 'interface' | 'defaults' | 'coding' | 'analysis' | 'ai' | 'team' | 'privacy' | 'accessibility' | 'updates' | 'about';

/** The sections of the book's Settings page, in its order. A section with nothing built yet says so instead of showing dead controls. */
const SECTIONS: { id: SectionId; label: string; icon: IconName }[] = [
  { id: 'general', get label() { return t('General'); }, icon: 'set-general' },
  { id: 'interface', get label() { return t('Interface'); }, icon: 'set-appearance' },
  { id: 'defaults', get label() { return t('Project Defaults'); }, icon: 'set-storage' },
  { id: 'coding', get label() { return t('Coding'); }, icon: 'cmd-code' },
  { id: 'analysis', get label() { return t('Analysis'); }, icon: 'exp-query' },
  { id: 'ai', get label() { return t('AI Assistant'); }, icon: 'cmd-ai-assist' },
  { id: 'team', get label() { return t('Team & Sharing'); }, icon: 'ws-team' },
  { id: 'privacy', get label() { return t('Privacy & Security'); }, icon: 'set-privacy' },
  { id: 'accessibility', get label() { return t('Accessibility'); }, icon: 'set-accessibility' },
  { id: 'updates', get label() { return t('Updates'); }, icon: 'set-updates' },
  { id: 'about', get label() { return t('About'); }, icon: 'set-about' },
];

const FOLDER_KEY = 'prismqda.projectFolder';

/**
 * Settings: how PrismQDA looks and what it says aloud, where new projects go, and how many backups are kept. These belong
 * to the person, not to a study, so they stay on this computer and every project opens the same way. Only what works is
 * here; a section whose screen is not built says so.
 */
export function SettingsView() {
  const [section, setSection] = useState<SectionId>('general');
  const current = SECTIONS.find((s) => s.id === section)!;

  return (
    <main id="main-content" tabIndex={-1} className="view-page bk-settings-page" aria-labelledby="h-settings">
      <h2 id="h-settings">{t('Settings')}</h2>
      <div className="bk-settings">
        <nav aria-label={t('Settings sections')} className="bk-settings-nav">
          <ul>
            {SECTIONS.map((s) => (
              <li key={s.id}>
                <Button className="bk-settings-item" aria-current={s.id === section ? 'page' : undefined} onPress={() => setSection(s.id)}>
                  <Icon name={s.icon} size={20} />
                  <span>{s.label}</span>
                </Button>
              </li>
            ))}
          </ul>
        </nav>

        <section className="bk-settings-panel" aria-labelledby="h-set-section" data-testid="settings-section">
          <h3 id="h-set-section" className="bk-settings-title">
            {current.label}
          </h3>
          {section === 'general' ? <General /> : null}
          {section === 'interface' ? <InterfaceSection /> : null}
          {section === 'defaults' ? <ProjectDefaults /> : null}
          {section === 'coding' ? <CodingSection /> : null}
          {section === 'analysis' ? <AnalysisSection /> : null}
          {section === 'ai' ? <AiSection /> : null}
          {section === 'team' ? <TeamSection /> : null}
          {section === 'privacy' ? <Privacy /> : null}
          {section === 'accessibility' ? <Accessibility /> : null}
          {section === 'updates' ? <Updates /> : null}
          {section === 'about' ? <About /> : null}

          <div className="bk-settings-foot">
            <Button onPress={() => resetPanelSizes()}>{t('Reset panel sizes')}</Button>
            <Button onPress={() => resetSettings()}>{t('Reset all settings')}</Button>
          </div>
        </section>
      </div>
    </main>
  );
}

function Scope({ children }: { children: ReactNode }) {
  return <span className="bk-scope">{children}</span>;
}

function Group({ id, title, scope, children }: { id: string; title: string; scope?: string; children: ReactNode }) {
  return (
    <section className="bk-set-group" aria-labelledby={id}>
      <h4 id={id}>
        {title}
        {scope ? <Scope>{scope}</Scope> : null}
      </h4>
      {children}
    </section>
  );
}

function Check({ selected, onChange, children, disabled }: { selected: boolean; onChange: (v: boolean) => void; children: ReactNode; disabled?: boolean }) {
  return (
    <Checkbox isSelected={selected} onChange={onChange} isDisabled={disabled} className="check">
      <div className="box" aria-hidden="true">
        <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
          <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
        </svg>
      </div>
      {children}
    </Checkbox>
  );
}

const PAIR_LABELS: Record<PairMethod, () => string> = {
  tukey: () => t("Tukey's honestly significant difference (groups of similar spread)"),
  games_howell: () => t('Games-Howell (groups of different spread or size)'),
  scheffe: () => t('Scheff\u00e9 (the most cautious: holds for every comparison of groups)'),
  holm: () => t('Holm (never weaker than Bonferroni, and as safe)'),
  bonferroni: () => t('Bonferroni (each p times the number of pairs)'),
  benjamini_hochberg: () => t('Benjamini-Hochberg (controls the share of false findings)'),
  none: () => t('No adjustment'),
};

function AnalysisSection() {
  const settings = useApp((s) => s.settings);
  const [seed, setSeed] = useState(settings.fixedSeed === null ? '' : String(settings.fixedSeed));
  return (
    <>
      <p className="muted">{t('These are the choices the analysis pages begin with. Each page lets you change them for the analysis in front of you; here you change where it begins. They belong to this computer and not to a project.')}</p>
      <Group id="h-set-scope" title={t('What the Statistics page offers')} scope={t('This computer')}>
        <RadioGroup value={settings.statisticsScope} onChange={(v) => updateSettings({ statisticsScope: v as StatisticsScope })} className="field" aria-labelledby="h-set-scope" data-testid="analysis-scope">
          <Radio value="descriptive">
            <span className="radio-title">{t('Describing the data (the usual)')}</span>
            <span className="radio-help">{t('Counts, shares, averages, spread, and charts. The tests are done in PrismStatistics, to which Export sends the case table.')}</span>
          </Radio>
          <Radio value="tests">
            <span className="radio-title">{t('Describing and testing')}</span>
            <span className="radio-help">{t('Also the tests of difference, association, and prediction (t tests, analysis of variance, regression, and the rest), as before.')}</span>
          </Radio>
        </RadioGroup>
      </Group>
      <Group id="h-set-level" title={t('Confidence level of intervals')} scope={t('This computer')}>
        <RadioGroup value={String(settings.confidenceLevel)} onChange={(v) => updateSettings({ confidenceLevel: Number(v) })} className="field" aria-labelledby="h-set-level" data-testid="analysis-level">
          {CONFIDENCE_LEVELS.map((l) => (
            <Radio key={l} value={String(l)}>
              <span className="radio-title">{l === 0.95 ? t('{level} (the usual)', { level: levelText(l) }) : levelText(l)}</span>
            </Radio>
          ))}
        </RadioGroup>
      </Group>
      <Group id="h-set-pairs" title={t('Comparing pairs of groups')} scope={t('This computer')}>
        <RadioGroup value={settings.pairMethod} onChange={(v) => updateSettings({ pairMethod: v as PairMethod })} className="field" aria-labelledby="h-set-pairs" data-testid="analysis-pairs">
          {PAIR_METHOD_IDS.map((id) => (
            <Radio key={id} value={id}>
              <span className="radio-title">{PAIR_LABELS[id]()}</span>
            </Radio>
          ))}
        </RadioGroup>
        <p className="muted">{t('Tukey, Games-Howell, and Scheff\u00e9 are made for the pairs after an analysis of variance; after Kruskal-Wallis, and when one of them is not offered, an adjustment of the p-values is used instead (Holm if you chose one of the three).')}</p>
      </Group>
      <Group id="h-set-boot" title={t('Bootstrap')} scope={t('This computer')}>
        <RadioGroup value={String(settings.bootstrapResamples)} onChange={(v) => updateSettings({ bootstrapResamples: Number(v) })} className="field" aria-labelledby="h-set-boot" data-testid="analysis-resamples">
          {RESAMPLE_CHOICES.map((n) => (
            <Radio key={n} value={String(n)}>
              <span className="radio-title">{t('{n} resamples', { n: num(n) })}</span>
            </Radio>
          ))}
        </RadioGroup>
        <Check selected={settings.fixedSeed !== null} onChange={(v) => updateSettings({ fixedSeed: v ? (Number(seed) >= 0 && seed.trim() !== '' ? Math.floor(Number(seed)) : 1) : null })}>
          {t('Begin every bootstrap with the same seed, so that a result can be repeated')}
        </Check>
        {settings.fixedSeed !== null ? (
          <div className="field">
            <label htmlFor="analysis-seed">{t('Seed')}</label>
            <input
              id="analysis-seed"
              type="number"
              min="0"
              step="1"
              value={seed === '' ? String(settings.fixedSeed) : seed}
              onChange={(e) => {
                setSeed(e.target.value);
                const n = Math.floor(Number(e.target.value));
                if (e.target.value.trim() !== '' && Number.isFinite(n) && n >= 0) updateSettings({ fixedSeed: n });
              }}
              data-testid="analysis-seed"
            />
          </div>
        ) : null}
        <p className="muted">{t('Without a fixed seed, each project begins with a new one, shown on the page so that the run can be repeated.')}</p>
      </Group>
      <Group id="h-set-sub" title={t('Queries, matrices, and relations')} scope={t('This computer')}>
        <Check selected={settings.subcodes} onChange={(v) => updateSettings({ subcodes: v })}>{t('A code also stands for its sub-codes')}</Check>
        <p className="muted">{t('This is where a new query, a code matrix, and a table of code relations begin; each lets you change it for the question in front of you.')}</p>
      </Group>
    </>
  );
}

function NotBuilt({ what, detail }: { what: string; detail: string }) {
  return (
    <div className="bk-set-empty">
      <p>
        <strong>{t('{what} are not built yet.', { what })}</strong>
      </p>
      <p className="muted">{detail}</p>
    </div>
  );
}

// ---------------------------------------------------------------- AI Assistant

/** What the AI route is and is not, and the one switch that turns it off for the open project. */
function AiSection() {
  const project = useApp((s) => s.project);
  const mode = project?.ai_mode ?? 'packet';
  const locked = project === null || project.read_only;
  return (
    <>
      <Group id="h-set-ai-how" title={t('How AI is used')}>
        <p className="bk-set-state">
          <Icon name="st-offline" size={18} />
          <span>{t('PrismQDA never connects to an AI service.')}</span>
        </p>
        <p className="muted">{t('The one way to use an AI is a prompt packet: text built from your codebook and some of your passages, which you copy into a chatbot you choose and bring the answer back from. What leaves this computer is only what you paste into another program. Every quote in an answer is checked against your documents, and each suggestion waits in the review queue until you accept it.')}</p>
      </Group>
      <Group id="h-set-ai-allow" title={t('Prompt packets')} scope={t('This project')}>
        <Check selected={mode !== 'off'} onChange={(on) => void setAiMode(on ? 'packet' : 'off')} disabled={locked}>{t('Allow prompt packets in this project')}</Check>
        <p className="muted">{t('When this is off no packet can be built. The record of packets already built is kept.')}</p>
        <div className="actions">
          <Button onPress={() => goTo('ai')} data-testid="settings-open-ai">{t('Open the AI Assistant')}</Button>
        </div>
      </Group>
      <Group id="h-set-ai-never" title={t('What is never put in a packet')} scope={t('This project')}>
        <ul className="bk-set-list">
          <li>{t('A document marked as one that must never be sent. Mark documents on the AI Assistant page, under Documents that must never be sent.')}</li>
          <li>{t('A code with AI Suggestible turned off, or one marked do not share. AI Suggestible is a Code Property in the Codebook Editor.')}</li>
          <li>{t('Passages you have already coded, unless you ask for them, and passages too long to quote exactly.')}</li>
        </ul>
      </Group>
    </>
  );
}

// ---------------------------------------------------------------- Team and sharing

/** Who has coded in the open project, told as it is; working together on one project is not built and says so. */
function TeamSection() {
  const project = useApp((s) => s.project);
  const coders = useApp((s) => s.coders);
  return (
    <>
      <Group id="h-set-coders" title={t('Coders in this project')} scope={t('This project')}>
        {project ? (
          coders.length === 0 ? (
            <p className="muted">{t('Nobody has coded in this project yet. You are recorded as a coder with your first coding.')}</p>
          ) : (
            <ul className="bk-coder-list" data-testid="coders-list">
              {coders.map((c) => (
                <li key={c.id}>
                  <strong>{c.name}</strong>
                  {c.id === project.me ? ' (you)' : ''}
                  {c.role === 'model' ? (c.model_name ? t(', a model: {name}', { name: c.model_name }) : t(', a model')) : t(', a person')}
                </li>
              ))}
            </ul>
          )
        ) : (
          <p className="muted">{t('Open a project to see who has coded in it.')}</p>
        )}
        <p className="muted">{t('Every change is recorded under the name of the coder who made it. The name comes from the user account of this computer.')}</p>
      </Group>
      <Group id="h-set-share" title={t('Sharing')} scope={t('This project')}>
        <p className="muted">{t('A project is one file on this computer. Coders who worked apart on copies of it can bring their work back together, compare how they coded, and settle their differences on the Team page.')}</p>
        <div className="actions">
          <Button onPress={() => goTo('team')} data-testid="settings-open-team">{t('Open the Team page')}</Button>
        </div>
      </Group>
    </>
  );
}

// ---------------------------------------------------------------- Coding

/** What belongs to coding: the one switch that is the person's own, and the rules that are set in the project, told as they stand. */
function CodingSection() {
  const settings = useApp((s) => s.settings);
  const project = useApp((s) => s.project);
  const codes = useApp((s) => s.codes);
  const groups = new Map<string, string[]>();
  for (const c of codes) {
    if (c.status !== 'active' || c.exclusive_group === '') continue;
    const key = c.exclusive_group.toLowerCase();
    groups.set(key, [...(groups.get(key) ?? []), c.name]);
  }
  const rated = codes.filter((c) => c.status === 'active' && c.rated);
  return (
    <>
      <Group id="h-set-bar" title={t('Coding by mouse')} scope={t('This computer')}>
        <Check selected={settings.floatingBar} onChange={(v) => updateSettings({ floatingBar: v })}>{t('Show the coding bar above words I select with the mouse')}</Check>
        <p className="muted">{t('The bar offers the codes you used last and your favorites. It appears only for the mouse; selecting with the keyboard never shows it, and the keys and the Codes list code the same with or without it.')}</p>
      </Group>

      <Group id="h-set-rules" title={t('The rules of this project')} scope={t('This project')}>
        {project ? (
          <>
            <dl className="bk-facts" data-testid="coding-rules">
              <div>
                <dt>{t('How it codes')}</dt>
                <dd>{project.coding_mode === 'unit' ? t('Whole units: every coding is a sentence, a speaker turn, or an answer.') : t('Any selected words.')}</dd>
              </div>
              <div>
                <dt>{t('Mutually exclusive groups')}</dt>
                <dd>
                  {groups.size === 0
                    ? t('None. Any codes can share the same words.')
                    : [...groups.values()].map((names, i) => <span key={i}>{i > 0 ? '; ' : ''}{names.join(', ')}</span>)}
                </dd>
              </div>
              <div>
                <dt>{t('Rated codes')}</dt>
                <dd>{rated.length === 0 ? t('None. Codings are not rated.') : rated.map((c) => c.name).join(', ')}</dd>
              </div>
            </dl>
            <p className="muted">{t('A group and a rating scale belong to each code, so they are set in the Codebook Editor, and they travel with the project file. ')}
              <Button className="bk-link" onPress={() => goTo('codebook')}>{t('Open the Codebook Editor')}</Button>
            </p>
          </>
        ) : (
          <p className="muted">{t('Open a project to see how it codes. Whether it codes any words or whole units is chosen when the project is made.')}</p>
        )}
      </Group>
    </>
  );
}

// ---------------------------------------------------------------- General

function General() {
  const settings = useApp((s) => s.settings);
  return (
    <>
      <Group id="h-set-location" title={t('Default Project Location')} scope={t('This computer')}>
        <DefaultLocation />
      </Group>

      <Group id="h-set-autosave" title={t('Autosave')}>
        <p className="bk-set-state" data-testid="autosave-state">
          <Icon name="st-saved" size={18} />
          <span>{t('Every change is saved to the project file as you make it.')}</span>
        </p>
        <p className="muted">{t('There is no timer and nothing to remember to save, so a crash or a power cut cannot lose more than the change in progress. If a save fails, the status bar says so.')}</p>
      </Group>

      <Group id="h-set-backup" title={t('Backup')} scope={t('This computer')}>
        <div className="field">
          <label htmlFor="set-keep">{t('Recent backups kept for each project')}</label>
          <select id="set-keep" value={settings.keepBackups} onChange={(e) => updateSettings({ keepBackups: Number(e.target.value) })}>
            {KEEP_BACKUP_CHOICES.map((k) => (
              <option key={k} value={k}>
                {t('{count} backups', { count: k })}</option>
            ))}
          </select>
        </div>
        <BackupLocation />
        <p className="muted">{rich('A dated copy is made each time a project is opened (and when you choose Backup), in a folder named after the project and ending in <code>.backups</code>. By default that folder is beside the project file; if you choose another folder, each project gets its own folder inside it. The oldest copies beyond the number above are removed. A backup is an ordinary project file: open it like any other. If a backup cannot be made where you chose, the project still opens and is protected beside itself, and a notice says why.', { code: (inside) => <code>{inside}</code> })}</p>
      </Group>

      <Group id="h-set-history" title={t('Your name in the project history')} scope={t('This computer')}>
        <HistoryName />
      </Group>

      <Group id="h-set-language" title={t('Language')} scope={t('This computer')}>
        <LanguageBody />
      </Group>

      <Group id="h-set-theme" title={t('Theme')} scope={t('This computer')}>
        <ThemeChoice />
      </Group>
    </>
  );
}

/**
 * The name the person goes by in the history of a project. Every change is recorded under it and the person is a coder under it. Left empty,
 * the engine uses the name the program made for this computer. It is never the Windows account name, which a project or a copy that is
 * shared would carry on every change.
 */
function HistoryName() {
  const saved = useApp((s) => s.settings.historyName);
  const [text, setText] = useState(saved);
  useEffect(() => setText(saved), [saved]);
  const commit = () => {
    const clean = cleanHistoryName(text);
    setText(clean);
    if (clean !== saved) updateSettings({ historyName: clean });
  };
  return (
    <>
      <div className="field">
        <label htmlFor="set-history-name">{t('Name')}</label>
        <input
          id="set-history-name"
          data-testid="history-name"
          type="text"
          maxLength={60}
          value={text}
          placeholder={t('The name this program made for you')}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              commit();
            }
          }}
        />
      </div>
      <p className="muted">{t('Every change you make is recorded in the project under this name, and you appear under it as a coder. Leave it empty to use the name this program made for this computer. It is never your Windows sign-in name. People who each work on their own copy of a project and merge them later should each use a different name. A copy made to be shared replaces the names with Researcher 1, Researcher 2 and so on. A new name applies from now on; what was recorded before keeps the name it had.')}</p>
    </>
  );
}

/**
 * Where backups go: beside each project (the standard) or in a folder the person chooses. A folder is kept only after the
 * engine has checked that it exists and can be written in; when it cannot, the last usable choice stays and the reason is said.
 */
function BackupLocation() {
  const saved = useApp((s) => s.settings.backupFolder);
  const [mode, setMode] = useState<'project' | 'folder'>(saved === '' ? 'project' : 'folder');
  const [value, setValue] = useState(saved);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const stays = saved === '' ? t('Backups stay beside each project.') : t('Backups stay in {folder}.', { folder: saved });

  const commit = async (raw: string) => {
    const typed = cleanTypedPath(raw);
    setNote(null);
    if (typed === saved) {
      setError(null);
      setValue(typed);
      return;
    }
    if (typed === '') {
      setError(t('Type or choose a folder. {stays}', { stays }));
      return;
    }
    try {
      await setBackupFolder(typed);
      setValue(typed);
      setError(null);
      setNote(t('Saved. Backups of every project now go in {typed}.', { typed }));
    } catch (e) {
      const message = `${toApiError(e).message} ${stays}`;
      setError(message);
      announce(message, 'assertive');
    }
  };

  const chooseMode = async (next: 'project' | 'folder') => {
    setMode(next);
    setError(null);
    setNote(null);
    if (next === 'project' && saved !== '') {
      try {
        await setBackupFolder('');
        setValue('');
        setNote(t('Saved. Backups are made beside each project.'));
      } catch (e) {
        setError(toApiError(e).message);
      }
    }
  };

  return (
    <>
      <div className="field">
        <label htmlFor="set-backup-where">{t('Backup location')}</label>
        <select id="set-backup-where" value={mode} onChange={(e) => void chooseMode(e.target.value as 'project' | 'folder')}>
          <option value="project">{t('Project folder (recommended)')}</option>
          <option value="folder">{t('A folder I choose')}</option>
        </select>
      </div>
      {mode === 'folder' ? (
        <TextField
          value={value}
          onChange={(v) => {
            setValue(v);
            setNote(null);
          }}
          onBlur={() => void commit(value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void commit(value);
            }
          }}
          className="field"
          isInvalid={error !== null}
          validationBehavior="aria"
        >
          <Label>{t('Folder for backups')}</Label>
          <div className="bk-location-row">
            <Input placeholder={t('D:\\Backups\\PrismQDA')} />
            {nativeDialogsAvailable ? (
              <Button
                type="button"
                aria-label={t('Browse for the backup folder')}
                onPress={async () => {
                  const f = await browsePath('project-folder');
                  if (f) void commit(f);
                }}
              >{t('Browse…')}</Button>
            ) : null}
          </div>
          <Text slot="description" className="muted">
            <span data-testid="backup-note">{error ? null : (note ?? t('Press Enter or move on to keep a change. The folder must exist and accept new files.'))}</span>
          </Text>
          {error ? (
            <div role="alert" className="bk-field-error" data-testid="backup-error">
              {error}
            </div>
          ) : null}
        </TextField>
      ) : note ? (
        <p className="muted" role="status" data-testid="backup-note">
          {note}
        </p>
      ) : null}
    </>
  );
}

function DefaultLocation() {
  const [value, setValue] = useState(() => {
    const v = readJson<unknown>(FOLDER_KEY, '');
    return typeof v === 'string' ? v : '';
  });
  const [saved, setSaved] = useState(value);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const keep = saved === '' ? t('No default is set, so New Project asks for a folder each time.') : t('The default stays {folder}.', { folder: saved });

  // A change is kept only if it names a folder that exists; otherwise the last usable folder stays.
  // `asked` is true when the person pressed Enter: they get an answer even when nothing changed.
  const commit = async (raw: string, asked = false) => {
    const typed = cleanTypedPath(raw);
    setNote(null);
    if (typed === saved) {
      setError(null);
      setValue(typed);
      if (asked && typed !== '') {
        setNote(t('This folder is already the default. New projects will be offered it.'));
        announce(t('{typed} is already the default project location.', { typed }));
      }
      return;
    }
    if (typed === '') {
      writeJson(FOLDER_KEY, '');
      setSaved('');
      setValue('');
      setError(null);
      setNote(t('The default location is cleared.'));
      announce(t('The default project location is cleared.'));
      return;
    }
    try {
      const st = await call('file.stat', { path: typed });
      if (!st.exists) {
        setError(t('{typed} was not found. {keep}', { typed, keep }));
        announce(t('That folder was not found. {keep}', { keep }), 'assertive');
        return;
      }
      if (st.is_file) {
        setError(t('{typed} is a file, not a folder. {keep}', { typed, keep }));
        announce(t('That is a file, not a folder. {keep}', { keep }), 'assertive');
        return;
      }
      writeJson(FOLDER_KEY, typed);
      setSaved(typed);
      setValue(typed);
      setError(null);
      setNote(t('Saved. New projects will be offered this folder.'));
      announce(t('The default project location is now {typed}.', { typed }));
    } catch (e) {
      const m = toApiError(e).message;
      setError(`${m} ${keep}`);
      announce(t('The default location could not be checked. {keep}', { keep }), 'assertive');
    }
  };

  return (
    <TextField
      value={value}
      onChange={(v) => {
        setValue(v);
        setNote(null);
      }}
      onBlur={() => void commit(value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          void commit(value, true);
        }
      }}
      className="field"
      isInvalid={error !== null}
      validationBehavior="aria"
    >
      <Label>{t('Folder new projects are saved in')}</Label>
      <div className="bk-location-row">
        <Input placeholder={t('C:\\Users\\you\\Documents\\PrismQDA Projects')} />
        {nativeDialogsAvailable ? (
          <Button
            type="button"
            aria-label={t('Browse for the default folder')}
            onPress={async () => {
              const f = await browsePath('project-folder');
              if (f) void commit(f);
            }}
          >{t('Browse…')}</Button>
        ) : null}
      </div>
      <Text slot="description" className="muted">
        <span data-testid="location-note">{error ? null : (note ?? t('Press Enter or move on to keep a change. The folder must already exist.'))}</span>
      </Text>
      {error ? (
        <div role="alert" className="bk-field-error" data-testid="location-error">
          {error}
        </div>
      ) : null}
    </TextField>
  );
}

/** Four cards, each a small drawing of the colors it gives; the radio inside is what a screen reader meets. */
function ThemeChoice() {
  const settings = useApp((s) => s.settings);
  return (
    <>
      <RadioGroup value={settings.theme} onChange={(v) => updateSettings({ theme: v as Theme })} className="bk-themes" aria-label={t('Theme')}>
        {THEMES.map((t) => (
          <Radio key={t.id} value={t.id} className="react-aria-Radio bk-theme-card">
            <span className="bk-theme-preview" data-preview={t.id} aria-hidden="true">
              <span className="bk-tp-bar" />
              <span className="bk-tp-line a" />
              <span className="bk-tp-line b" />
              <span className="bk-tp-line c" />
            </span>
            <span className="bk-theme-name">{t.label}</span>
            <span className="visually-hidden">. {t.help}</span>
          </Radio>
        ))}
      </RadioGroup>
      <p className="muted">{t('The colors change at once. Windows high contrast mode is also followed, whatever is chosen here.')}</p>
    </>
  );
}

// ---------------------------------------------------------------- Interface

function InterfaceSection() {
  const settings = useApp((s) => s.settings);
  return (
    <>
      <Group id="h-set-size" title={t('Text size')} scope={t('This computer')}>
        <RadioGroup value={String(settings.textSize)} onChange={(v) => updateSettings({ textSize: Number(v) })} className="field" aria-labelledby="h-set-size">
          {TEXT_SIZES.map((size) => (
            <Radio key={size} value={String(size)}>
              <span className="radio-title">{size === 100 ? t('100 percent (standard)') : t('{size} percent', { size })}</span>
            </Radio>
          ))}
        </RadioGroup>
        <p className="settings-sample" data-testid="text-sample">{rich('Interviewer: How did the first week go?<x1/>Maya: I was nervous at first, but the staff greeted me by name and I felt welcome right away.', { x1: <br /> })}</p>
        <p className="muted">{t('Everything in the window grows or shrinks with this, including the buttons and the text you code.')}</p>
      </Group>

      <Group id="h-set-read" title={t('Reading the text')} scope={t('This computer')}>
        <Check selected={settings.lineNumbers} onChange={(v) => updateSettings({ lineNumbers: v })}>{t('Number the lines of text')}</Check>
        <p className="muted">{t('Lines are counted as they are drawn, so the numbers change when the window or the text size changes. They help you find your place and read "lines 7 to 9"; the project itself keeps units and character positions.')}</p>
        <Check selected={settings.showUnits} onChange={(v) => updateSettings({ showUnits: v })}>{t('Show where each unit starts')}</Check>
        <p className="muted">{t("A thin line and the speaker's name at the start of each sentence, turn, or answer. Needed to see the units a project that codes whole units works on.")}</p>
        <RadioGroup value={settings.textWidth} onChange={(v) => updateSettings({ textWidth: v as TextWidth })} className="field" data-testid="text-width">
          <Label>{t('How wide the text runs')}</Label>
          <Radio value="fill">
            <span className="radio-text">
              {rich('<span>Fill the window</span><span2>Lines run across all the room the middle of the window has.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
            </span>
          </Radio>
          <Radio value="comfortable">
            <span className="radio-text">
              {rich('<span>Comfortable line length</span><span2>Lines stop at about 70 characters, which many people find easier to read on a wide screen.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
            </span>
          </Radio>
        </RadioGroup>
      </Group>
    </>
  );
}

// ---------------------------------------------------------------- Project Defaults

function ProjectDefaults() {
  const settings = useApp((s) => s.settings);
  return (
    <Group id="h-set-mode" title={t('How new projects code')} scope={t('New projects')}>
      <RadioGroup value={settings.codingMode} onChange={(v) => updateSettings({ codingMode: v as 'free_span' | 'unit' })} className="field" aria-labelledby="h-set-mode">
        <Radio value="free_span">
          <span className="radio-text">
            {rich('<span>Select any words</span><span2>Code exactly the words you select. Good for interviews and documents.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
          </span>
        </Radio>
        <Radio value="unit">
          <span className="radio-text">
            {rich('<span>Code whole units</span><span2>Code each sentence, speaker turn, or survey answer as a whole. Needed for counting and agreement.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
          </span>
        </Radio>
      </RadioGroup>
      <p className="muted">{t('This is only the starting choice on the New Project page. A project keeps the way it was made to code, whatever is chosen here later. The folder new projects go in is under General.')}</p>
    </Group>
  );
}

// ---------------------------------------------------------------- Privacy, Accessibility, Updates, About

function Privacy() {
  return (
    <>
      <Group id="h-set-local" title={t('Where your data is')} scope={t('This computer')}>
        <p className="bk-set-state">
          <Icon name="st-offline" size={18} />
          <span>{t('PrismQDA connects to nothing on its own. The one exception is the Check for updates button in Settings, which you press.')}</span>
        </p>
        <p className="muted">{t("A project is one file on this computer. Nothing is uploaded, and there is no account. The installer carries Microsoft's WebView2 setup for a computer that does not have it (Windows 11 does), so it downloads nothing.")}</p>
      </Group>
      <Group id="h-set-encrypt" title={t('Protecting a project')} scope={t('This project')}>
        <EncryptionPanel />
      </Group>
    </>
  );
}

function Accessibility() {
  const settings = useApp((s) => s.settings);
  return (
    <>
      <Group id="h-set-sr" title={t('Screen reader')} scope={t('This computer')}>
        <Check selected={settings.announceCaret} onChange={(v) => updateSettings({ announceCaret: v })}>{t('Say which codes the caret is in as it moves through the text')}</Check>
        <p className="muted">{t('A short note such as "In Welcome, Cost." when the caret enters or leaves coded words. Turn it off if it talks over what you are reading.')}</p>
      </Group>
      <Group id="h-set-keys" title={t('Keyboard and display')}>
        <ul className="bk-set-list">
          <li>{t('Every control can be reached with Tab, and a ring shows where you are. F6 moves between the main areas, Shift+F6 goes back, and F1 lists the shortcuts.')}</li>
          <li>{t('Text size is under Interface, and the High Contrast colors are under Theme in General. Windows high contrast mode is followed whatever is chosen.')}</li>
          <li>{t('A code is never told apart by color alone: its name is in the tooltip, in Codings here, and in Segment Details.')}</li>
        </ul>
      </Group>
    </>
  );
}

function Updates() {
  return (
    <Group id="h-set-updates" title={t('Program updates')}>
      <UpdatesCard />
    </Group>
  );
}

function About() {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    // The program's own version (the shell says it, and it is what updates are compared with); the engine's when the page is not in the program.
    call('update.state')
      .then((s) => live && setVersion(s.version))
      .catch(() =>
        call('ping')
          .then((p) => live && setVersion(p.version))
          .catch(() => live && setVersion(null)),
      );
    return () => {
      live = false;
    };
  }, []);
  return (
    <>
      <Group id="h-set-about" title={t('PrismQDA')}>
        <p data-testid="about-version">{t('Version {version}, a beta. A free, offline Windows tool for qualitative and mixed-methods analysis.', { version: version ?? '…' })}</p>
        <p className="muted">{t('Licensed under the Apache License 2.0. The interface uses the Inter typeface under the SIL Open Font License; both notices are installed beside the program.')}</p>
      </Group>
      <Group id="h-set-stored" title={t('Where settings are kept')}>
        <p className="muted">{t('These settings are stored on this computer, not in any project, so every project opens the same way. Reset all settings puts them back to how they were when the program was installed.')}</p>
      </Group>
    </>
  );
}
