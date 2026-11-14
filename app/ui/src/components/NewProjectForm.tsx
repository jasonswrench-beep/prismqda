import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { Button, FieldError, Input, Label, Radio, RadioGroup, Text, TextArea, TextField } from 'react-aria-components';
import { call, inTauri, toApiError } from '../bridge';
import { freeName, splitPath, targetPath, validateProjectName } from '../lib/projectPath';
import { readJson, writeJson } from '../lib/storage';
import { createProject } from '../state/actions';
import { announce } from '../state/announce';
import { appStore } from '../state/app';
import { browsePath, nativeDialogsAvailable } from '../pickFile';
import type { TemplateRow } from '../types';
import { passphraseProblem } from '../lib/passphrase';
import { Icon } from './Icon';
import { LostPassphraseWarning, PassphraseFields } from './PassphraseFields';
import { Notice } from './common';
import { Tick } from './Tick';
import { rich, t, tn } from '../i18n';

const FOLDER_KEY = 'prismqda.projectFolder';

/** The folder new projects go to unless the person chooses another: the last one used, else Documents / PrismQDA Projects. */
async function defaultFolder(): Promise<string> {
  const last = readJson<unknown>(FOLDER_KEY, '');
  if (typeof last === 'string' && last !== '') return last;
  if (!inTauri()) return '';
  try {
    const { documentDir, join } = await import('@tauri-apps/api/path');
    return await join(await documentDir(), 'PrismQDA Projects');
  } catch {
    return '';
  }
}

/**
 * New Project Setup (screen specification book, Part 2): a name and a save location are enough to start. Where the file
 * will be written is shown before anything is created, a template can give the study a method's starting structure, and
 * the study details are optional and kept as a memo so they can be changed later. Cancelling leaves nothing behind.
 */
export function NewProjectForm(props: { onCancel: () => void; heading?: boolean }) {
  const [name, setName] = useState('');
  const [location, setLocation] = useState('');
  const [start, setStart] = useState<'blank' | 'template'>('blank');
  const [templates, setTemplates] = useState<TemplateRow[]>([]);
  const [templateId, setTemplateId] = useState('');
  const [open, setOpen] = useState(false);
  const [question, setQuestion] = useState('');
  const [method, setMethod] = useState('');
  const [unit, setUnit] = useState('');
  const [sampling, setSampling] = useState('');
  // New projects start with the way to code chosen in Settings (Project Defaults).
  const [mode, setMode] = useState<'free_span' | 'unit'>(() => appStore.get().settings.codingMode);
  const [nameError, setNameError] = useState<string | null>(null);
  const [locationError, setLocationError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [suggestion, setSuggestion] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Encrypting the project from the start: the passphrase is typed twice, and a warning window comes before the project is made.
  const [encrypt, setEncrypt] = useState(false);
  const [pass, setPass] = useState('');
  const [again, setAgain] = useState('');
  const [warn, setWarn] = useState(false);

  useEffect(() => {
    let live = true;
    void defaultFolder().then((f) => live && setLocation((cur) => (cur === '' ? f : cur)));
    call('template.list')
      .then((t) => live && setTemplates(t.filter((x) => x.id !== 'free')))
      .catch(() => live && setTemplates([]));
    return () => {
      live = false;
    };
  }, []);

  const template = templates.find((t) => t.id === templateId) ?? null;
  const target = useMemo(() => (location.trim() === '' ? '' : targetPath(location, name.trim() === '' ? t('My study') : name)), [location, name]);
  const where = target === '' ? null : splitPath(target);

  const submit = async (e: FormEvent, confirmed = false) => {
    e.preventDefault();
    setNameError(null);
    setLocationError(null);
    setFormError(null);
    setSuggestion(null);
    const bad = validateProjectName(name);
    if (bad) {
      setNameError(bad);
      return;
    }
    if (location.trim() === '') {
      setLocationError(t('Choose where to save the project.'));
      return;
    }
    if (start === 'template' && !template) {
      setFormError(t('Choose a method template, or start from a blank project.'));
      return;
    }
    if (encrypt) {
      const problem = passphraseProblem(pass, again);
      if (problem) {
        setFormError(problem);
        return;
      }
      if (!confirmed) {
        setWarn(true);
        return;
      }
    }
    setBusy(true);
    try {
      await createProject({
        name,
        path: target,
        mode,
        researchQuestions: question,
        ...(encrypt ? { passphrase: pass } : {}),
        ...(start === 'template' && template ? { method: template.id } : {}),
      });
      writeJson(FOLDER_KEY, splitPath(target).folder);
      // The details have no field of their own in the project file, so they are kept as a memo about the project, where they can be edited.
      const lines = [method.trim() && t('Method: {method}', { method: method.trim() }), unit.trim() && t('Unit of analysis: {unit}', { unit: unit.trim() }), sampling.trim() && t('Sampling notes: {sampling}', { sampling: sampling.trim() })].filter(Boolean);
      if (lines.length > 0) {
        try {
          await call('memo.add', { kind: 'method_decision', scope: 'project', title: t('Study details'), body: lines.join('\n') });
        } catch {
          announce(t('The project was created, but the study details could not be saved as a memo. Add them in a memo later.'), 'assertive');
        }
      }
    } catch (err) {
      const e2 = toApiError(err);
      setBusy(false);
      if (e2.code === 'already_exists' || /already exists/i.test(e2.message)) {
        setLocationError(t('A project already exists at {target}. Nothing was replaced. Choose another name or location.', { target }));
        setSuggestion(freeName(name.trim(), [name.trim()]));
      } else if (e2.code === 'io' || /file error|denied|permission|cannot find the path/i.test(e2.message)) {
        setLocationError(t('PrismQDA could not write there: {message}. Choose a folder you can save to.', { message: e2.message }));
      } else {
        setFormError(e2.message);
      }
    }
  };

  return (
    <form className="bk-newproject" onSubmit={(e) => void submit(e)} noValidate aria-labelledby="np-title">
      {props.heading === false ? null : (
        <header className="bk-np-head">
          <h2 id="np-title">{t('New Project')}</h2>
          <p className="muted">{t('Choose how to start your study and where to save it.')}</p>
        </header>
      )}
      {formError ? <Notice>{formError}</Notice> : null}

      <TextField value={name} onChange={setName} className="field" isInvalid={!!nameError} validationBehavior="aria" autoFocus>
        <Label>{t('Project name')}</Label>
        <Input placeholder={t('Urban Mobility Study')} />
        <FieldError>{nameError}</FieldError>
      </TextField>

      <div className="field bk-location">
        <TextField value={location} onChange={setLocation} className="bk-location-field" isInvalid={!!locationError} validationBehavior="aria">
          <Label>{t('Save location')}</Label>
          <div className="bk-location-row">
            <Input placeholder={t('C:\\Users\\you\\Documents\\PrismQDA Projects')} />
            {nativeDialogsAvailable ? (
              <Button
                type="button"
                aria-label={t('Browse for the folder to save in')}
                onPress={async () => {
                  const chosen = await browsePath('project-folder');
                  if (chosen) setLocation(chosen);
                }}
              >{t('Browse…')}</Button>
            ) : null}
          </div>
          <FieldError>{locationError}</FieldError>
          {where ? (
            <Text slot="description" className="bk-where" data-testid="np-where">{rich('Will be saved as <strong>{file}</strong> in {folder}.', { file: where.file, folder: where.folder || t('the current folder'), strong: (inside) => <strong>{inside}</strong> })}</Text>
          ) : (
            <Text slot="description">{t('Type the folder to save in, for example C:\\Users\\you\\Documents\\PrismQDA Projects.')}</Text>
          )}
        </TextField>
        {suggestion ? (
          <Button type="button" className="bk-link" onPress={() => setName(suggestion)}>{t('Use the name {suggestion}', { suggestion })}
          </Button>
        ) : null}
      </div>

      <RadioGroup value={start} onChange={(v) => setStart(v as 'blank' | 'template')} className="field bk-start" orientation="horizontal">
        <Label>{t('Start from')}</Label>
        <div className="bk-start-options">
          <Radio value="blank">{t('Blank project')}</Radio>
          <Radio value="template" isDisabled={templates.length === 0}>{t('Method template')}</Radio>
        </div>
      </RadioGroup>
      {start === 'template' ? (
        <div className="field bk-template">
          <label htmlFor="np-template">{t('Method template')}</label>
          <select id="np-template" value={templateId} onChange={(e) => setTemplateId(e.target.value)}>
            <option value="">{t('Choose a method…')}</option>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </select>
          {template ? (
            <p className="help" data-testid="np-template-summary">
              {template.summary}{' '}
              {t('Starts with {codes} and {phases}. A template holds no participant data.', { codes: tn(template.seed_codes.length, { one: '{count} code', other: '{count} codes' }), phases: tn(template.phases.length, { one: '{count} phase', other: '{count} phases' }) })}
            </p>
          ) : null}
        </div>
      ) : null}

      <div className="field enc-newproject">
        <Tick isSelected={encrypt} onChange={setEncrypt} testId="np-encrypt">{t('Encrypt this project with a passphrase')}</Tick>
        {encrypt ? (
          <PassphraseFields value={pass} again={again} onChange={(p, a) => { setPass(p); setAgain(a); }} idPrefix="np" />
        ) : (
          <p className="muted">{t('Choose this for interviews or other material that must not be read by anyone who finds the file. A lost passphrase cannot be recovered.')}</p>
        )}
      </div>

      <div className="bk-details">
        <button type="button" className="bk-details-toggle" aria-expanded={open} aria-controls="np-details" onClick={() => setOpen((o) => !o)}>
          <Icon name="ui-chevron-down" size={16} className={open ? 'open' : 'closed'} />{t('Optional study details')}</button>
        {open ? (
          <div id="np-details" className="bk-details-fields">
            <TextField value={question} onChange={setQuestion} className="field">
              <Label>{t('Research question')}</Label>
              <TextArea rows={2} placeholder={t('How do residents experience everyday public transit?')} />
            </TextField>
            <div className="bk-two">
              <TextField value={method} onChange={setMethod} className="field">
                <Label>{t('Method')}</Label>
                <Input placeholder={t('Qualitative interviews')} />
              </TextField>
              <TextField value={unit} onChange={setUnit} className="field">
                <Label>{t('Unit of analysis')}</Label>
                <Input placeholder={t('Interview passage')} />
              </TextField>
            </div>
            <TextField value={sampling} onChange={setSampling} className="field">
              <Label>{t('Sampling notes')}</Label>
              <TextArea rows={2} placeholder={t('Record recruitment or inclusion criteria')} />
            </TextField>
            <RadioGroup value={mode} onChange={(v) => setMode(v as 'free_span' | 'unit')} className="field">
              <Label>{t('How will you code?')}</Label>
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
            <p className="muted">{t('These details are kept as a memo called Study details, so you can change them later. The research question is kept with the project.')}</p>
          </div>
        ) : null}
      </div>

      {warn ? (
        <LostPassphraseWarning
          what={t('I understand. Create the encrypted project')}
          onCancel={() => setWarn(false)}
          onConfirm={() => {
            setWarn(false);
            void submit({ preventDefault: () => undefined } as FormEvent, true);
          }}
        />
      ) : null}

      <footer className="bk-np-foot">
        <span className="muted">{t('Local project')}</span>
        <div className="actions">
          <Button type="button" onPress={props.onCancel}>{t('Cancel')}</Button>
          <Button type="submit" className="primary" isDisabled={busy}>{t('Create Project')}</Button>
        </div>
      </footer>
    </form>
  );
}
