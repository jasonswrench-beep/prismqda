import { useEffect, useState, type FormEvent } from 'react';
import { Button, Dialog, FieldError, Heading, Input, Label, Modal, ModalOverlay, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { freeName, splitPath, targetPath, validateProjectName } from '../lib/projectPath';
import { readJson, writeJson } from '../lib/storage';
import { createSampleCopy, locateRecentProject, openProject } from '../state/actions';
import { appStore } from '../state/app';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import type { SampleRow } from '../types';
import { NewProjectForm } from './NewProjectForm';
import { Notice, plural } from './common';
import { rich, t, tn } from '../i18n';

const close = () => appStore.set({ dialog: null });

/** New Project from inside the open workspace: the same form as the welcome page. The open project stays open until the new one is made. */
export function NewProjectDialog() {
  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal bk-wide-modal">
        <Dialog className="dialog" aria-labelledby="np-title">
          <Heading slot="title" id="np-title" className="dialog-title">{t('New Project')}</Heading>
          <p className="muted">{t('Choose how to start your study and where to save it. The project that is open stays open until the new one is made.')}</p>
          <NewProjectForm onCancel={close} heading={false} />
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/**
 * Open Project: choose a project file, or type its path. The engine checks that it is a project, that it can be read, and
 * that its version is understood before the workspace changes; a refusal is shown here and the open project is untouched.
 */
export function OpenProjectDialog(props: { /** The recent entry being found again, if this is Locate. */ locate?: { path: string; name: string } }) {
  const [path, setPath] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const open = async (chosen: string) => {
    setError(null);
    setBusy(true);
    try {
      if (props.locate) await locateRecentProject(props.locate.path, chosen);
      else await openProject(chosen);
    } catch (e) {
      setError(toApiError(e).message);
      setBusy(false);
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const typed = cleanTypedPath(path);
    if (typed === '') {
      setError(t('Type the path of the project file, or press Browse.'));
      return;
    }
    void open(typed);
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal">
        <Dialog className="dialog" aria-labelledby="op-title">
          <Heading slot="title" id="op-title" className="dialog-title">
            {props.locate ? t('Find {name}', { name: props.locate.name }) : t('Open Project')}
          </Heading>
          {props.locate ? (
            <p className="muted">{rich('The project was not found at <code>{path}</code>. Choose where it is now. PrismQDA checks that the file is the same project before it updates the list.', { path: props.locate.path, code: (inside) => <code>{inside}</code> })}</p>
          ) : null}
          {error ? <Notice>{error}</Notice> : null}
          <form onSubmit={submit} noValidate>
            <TextField value={path} onChange={setPath} className="field" autoFocus>
              <Label>{t('Project file')}</Label>
              <div className="bk-location-row">
                <Input placeholder={t('C:\\Users\\you\\Documents\\PrismQDA Projects\\study.pqda')} />
                {nativeDialogsAvailable ? (
                  <Button
                    type="button"
                    aria-label={t('Browse for a project file to open')}
                    isDisabled={busy}
                    onPress={async () => {
                      const chosen = await browsePath('open-project');
                      if (chosen) {
                        setPath(chosen);
                        void open(chosen);
                      }
                    }}
                  >{t('Browse…')}</Button>
                ) : null}
              </div>
              <Text slot="description">{t('Type the full path of a project file you made before.')}</Text>
            </TextField>
            <div className="actions">
              <Button type="button" onPress={close}>{t('Cancel')}</Button>
              <Button type="submit" className="primary" isDisabled={busy}>
                {props.locate ? t('Find and open') : t('Open Project')}
              </Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/**
 * Sample Projects: a bundled study to practice on. Choosing one makes an editable copy at a place you choose, and opens
 * the copy. The bundled study itself can never be changed, so the sample is always as it was.
 */
export function SampleProjectsDialog() {
  const [samples, setSamples] = useState<SampleRow[] | null>(null);
  const [chosen, setChosen] = useState('');
  const [name, setName] = useState('');
  const [folder, setFolder] = useState(() => {
    const last = readJson<unknown>('prismqda.projectFolder', '');
    return typeof last === 'string' ? last : '';
  });
  const [error, setError] = useState<string | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    call('sample.list')
      .then((s) => {
        if (!live) return;
        setSamples(s);
        if (s[0]) {
          setChosen(s[0].id);
          setName(t('{name} (practice copy)', { name: s[0].name }));
        }
      })
      .catch((e) => live && (setSamples([]), setError(toApiError(e).message)));
    return () => {
      live = false;
    };
  }, []);

  const sample = samples?.find((s) => s.id === chosen) ?? null;
  const target = folder.trim() === '' ? '' : targetPath(folder, name || t('Practice copy'));

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setNameError(null);
    const bad = validateProjectName(name);
    if (bad) return setNameError(bad);
    if (!sample) return setError(t('Choose a sample.'));
    if (folder.trim() === '') return setError(t('Choose the folder for your practice copy.'));
    setBusy(true);
    try {
      await createSampleCopy(sample.id, target);
      writeJson('prismqda.projectFolder', splitPath(target).folder);
    } catch (err) {
      const m = toApiError(err);
      setBusy(false);
      if (m.code === 'already_exists' || /already exists/i.test(m.message)) {
        setNameError(t('A file already exists at {path}. Nothing was replaced. Try {name}.', { path: target, name: freeName(name, [name]) }));
      } else setError(m.message);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal bk-wide-modal">
        <Dialog className="dialog" aria-labelledby="sp-title">
          <Heading slot="title" id="sp-title" className="dialog-title">{t('Sample Projects')}</Heading>
          <p className="muted">{t('Practice on a made-up study. You get your own editable copy; the sample itself never changes.')}</p>
          {error ? <Notice>{error}</Notice> : null}
          {samples === null ? (
            <p role="status">{t('Looking for samples…')}</p>
          ) : samples.length === 0 ? (
            <p className="help">{t('No sample projects are available.')}</p>
          ) : (
            <form onSubmit={(e) => void submit(e)} noValidate>
              <RadioGroup
                value={chosen}
                onChange={(v) => {
                  setChosen(v);
                  const s = samples.find((x) => x.id === v);
                  if (s) setName(t('{name} (practice copy)', { name: s.name }));
                }}
                className="field bk-samples"
                aria-label={t('Sample project')}
              >
                {samples.map((s) => (
                  <Radio key={s.id} value={s.id}>
                    <span className="radio-text">
                      <span className="radio-title">{s.name}</span>
                      <span className="radio-help">{s.description}</span>
                      <span className="radio-help">
                        {tn(s.documents, { one: '{count} document', other: '{count} documents' })}, {tn(s.codes, { one: '{count} code', other: '{count} codes' })}, {tn(s.codings, { one: '{count} coding', other: '{count} codings' })}
                      </span>
                    </span>
                  </Radio>
                ))}
              </RadioGroup>
              <TextField value={name} onChange={setName} className="field" isInvalid={!!nameError} validationBehavior="aria">
                <Label>{t('Name of your copy')}</Label>
                <Input />
                <FieldError>{nameError}</FieldError>
              </TextField>
              <TextField value={folder} onChange={setFolder} className="field">
                <Label>{t('Save the copy in')}</Label>
                <div className="bk-location-row">
                  <Input placeholder={t('C:\\Users\\you\\Documents\\PrismQDA Projects')} />
                  {nativeDialogsAvailable ? (
                    <Button
                      type="button"
                      aria-label={t('Browse for the folder to save the copy in')}
                      onPress={async () => {
                        const f = await browsePath('project-folder');
                        if (f) setFolder(f);
                      }}
                    >{t('Browse…')}</Button>
                  ) : null}
                </div>
                {target ? (
                  <Text slot="description" className="bk-where">{rich('Will be saved as <strong>{file}</strong> in {folder}. It works offline.', { file: splitPath(target).file, folder: splitPath(target).folder, strong: (inside) => <strong>{inside}</strong> })}</Text>
                ) : null}
              </TextField>
              <div className="actions">
                <Button type="button" onPress={close}>{t('Cancel')}</Button>
                <Button type="submit" className="primary" isDisabled={busy}>{t('Make an editable copy')}</Button>
              </div>
            </form>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
