import { useMemo, useState, type FormEvent } from 'react';
import { Button, Checkbox, Dialog, Heading, Input, Label, Modal, ModalOverlay, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { announce } from '../state/announce';
import { appStore, useApp } from '../state/app';
import type { ExportFormat, QdpxReport, ReportSectionId } from '../types';
import { Notice } from './common';
import { QdpxReportView } from './QdpxReport';
import { NamesNotice } from './NamesNotice';
import { Tick } from './Tick';
import { num, rich, t } from '../i18n';

type What = 'codebook' | 'segments' | 'report' | 'project';

/** The parts of a report, in the order the report keeps them, with what each holds and whether it is in the report to begin with. */
const REPORT_SECTIONS: { id: ReportSectionId; label: string; help: string; on: boolean }[] = [
  { id: 'study', get label() { return t('The study'); }, get help() { return t('What the project is and how much is in it.'); }, on: true },
  { id: 'documents', get label() { return t('Documents'); }, get help() { return t('Each document, its kind, and how much it holds.'); }, on: true },
  { id: 'codebook', get label() { return t('Codebook'); }, get help() { return t('Every active code with its definition, rules, and examples.'); }, on: true },
  { id: 'passages', get label() { return t('Coded passages'); }, get help() { return t('The coded words grouped by code. Areas of pictures are shown cropped. This can be long.'); }, on: false },
  { id: 'agreement', get label() { return t('Agreement between coders'); }, get help() { return t('What each run of agreement reported.'); }, on: true },
  { id: 'adjudication', get label() { return t('Adjudicated decisions'); }, get help() { return t('The units coders coded differently and what was decided for each, with the reason.'); }, on: false },
  { id: 'memos', get label() { return t('Memos'); }, get help() { return t('Every memo, oldest first.'); }, on: false },
  { id: 'ai_use', get label() { return t('Use of AI'); }, get help() { return t('The paragraph about AI that a methods section needs.'); }, on: true },
  { id: 'methods', get label() { return t('Methods appendix (draft)'); }, get help() { return t('A draft built only from the project records: the data, the coding, the coders, agreement, and the software.'); }, on: true },
  { id: 'themes', get label() { return t('Themes'); }, get help() { return t('Each theme with where it stands, its definition, the codes under it with their counts, and quotations that show it.'); }, on: false },
  { id: 'ethics', get label() { return t('Ethics and data handling'); }, get help() { return t('Where the data is kept, whether it is encrypted, the offline lock, what was shared with an AI service, what is kept out, whether names were replaced, who coded, and the record of what was done: the facts an ethics board asks for.'); }, on: false },
  { id: 'mixed_methods', get label() { return t('Mixed methods'); }, get help() { return t('The design of the study, the integration log, and the triangulation matrix with your marks.'); }, on: false },
  { id: 'timeline', get label() { return t('Decision timeline'); }, get help() { return t('Notes, imports, and runs of agreement, by date.'); }, on: false },
];

const FORMATS: Record<What, { id: ExportFormat; label: string; help: string }[]> = {
  project: [{ id: 'qdpx', label: 'QDPX', get help() { return t('The REFI-QDA project exchange format: a ZIP of the project in XML and the files of its documents.'); } }],
  codebook: [
    { id: 'xlsx', get label() { return t('Excel'); }, get help() { return t('One row per code, with definitions, criteria, examples, and how often each is used.'); } },
    { id: 'docx', get label() { return t('Word'); }, get help() { return t('A readable document with a heading for each code, for sharing with a team or a committee.'); } },
    { id: 'qdc', get label() { return t('QDC codebook'); }, get help() { return t('The exchange format other qualitative analysis programs can read.'); } },
    { id: 'csv', label: 'CSV', get help() { return t('Plain table that any spreadsheet or statistics program opens.'); } },
  ],
  report: [{ id: 'docx', get label() { return t('Word'); }, get help() { return t('One document with a heading for each section, for a committee, a collaborator, or an appendix.'); } }],
  segments: [
    { id: 'xlsx', get label() { return t('Excel'); }, get help() { return t('One row per coded passage, with a second sheet that counts them by code and coder.'); } },
    { id: 'docx', get label() { return t('Word'); }, get help() { return t('The passages grouped by code, each with where it came from and who coded it.'); } },
    { id: 'csv', label: 'CSV', get help() { return t('One row per coded passage, for a spreadsheet or statistics program.'); } },
  ],
};

export function ExportDialog() {
  const project = useApp((s) => s.project);
  const codes = useApp((s) => s.codes);
  const coders = useApp((s) => s.coders);
  const asked = useApp((s) => (s.dialog?.kind === 'export' ? s.dialog.what : undefined));
  const presetSections = useApp((s) => (s.dialog?.kind === 'export' ? s.dialog.sections : undefined));
  const [what, setWhat] = useState<What>(asked ?? 'codebook');
  const [format, setFormat] = useState<ExportFormat>('xlsx');
  const [path, setPath] = useState('');
  const [codeId, setCodeId] = useState('');
  const [coderId, setCoderId] = useState('');
  const [finalLayer, setFinalLayer] = useState(false);
  const [withRecordings, setWithRecordings] = useState(false);
  const [inactive, setInactive] = useState(false);
  const [title, setTitle] = useState('');
  const [sections, setSections] = useState<ReportSectionId[]>(() => presetSections ?? REPORT_SECTIONS.filter((s) => s.on).map((s) => s.id));
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<{ path: string; bytes: number; report?: QdpxReport } | null>(null);
  const [busy, setBusy] = useState(false);

  const close = () => appStore.set({ dialog: null });
  const formats = FORMATS[what];
  const chosenFormat = formats.some((f) => f.id === format) ? format : formats[0].id;
  const activeCodes = useMemo(() => codes.filter((c) => c.status === 'active'), [codes]);
  const suggestion = `${(project?.name ?? 'PrismQDA').replace(/[\\/:*?"<>|]/g, '-')} ${what === 'project' ? t('project') : what === 'codebook' ? t('codebook') : what === 'report' ? t('report') : t('coded segments')}.${chosenFormat}`;

  const changeWhat = (w: What) => {
    setWhat(w);
    if (!FORMATS[w].some((f) => f.id === format)) setFormat(FORMATS[w][0].id);
  };

  const save = async (typed?: string, fromDialog = false) => {
    setError(null);
    setSaved(null);
    const target = cleanTypedPath(typed ?? path);
    if (target === '') {
      const picked = await browsePath('export-file', suggestion, chosenFormat);
      if (!picked) return setError(t('Type where to save the file, for example C:\\Reports\\codebook.xlsx'));
      return save(picked, true);
    }
    setBusy(true);
    try {
      const common = {
        format: chosenFormat,
        path: target,
        // A path chosen in Windows' own save dialog has already been confirmed; a typed one never replaces a file.
        overwrite: fromDialog,
      };
      if (what === 'report' && sections.length === 0) {
        setBusy(false);
        return setError(t('Choose at least one section for the report.'));
      }
      const r =
        what === 'project'
          ? await call('export.qdpx', { path: target, overwrite: fromDialog, ...(withRecordings ? { include_recordings: true } : {}) })
          : what === 'codebook'
          ? await call('export.codebook', { ...common, include_inactive: inactive })
          : what === 'report'
            ? await call('export.report', {
                path: target,
                overwrite: fromDialog,
                ...(title.trim() ? { title: title.trim() } : {}),
                // Kept in the order the report has them, whatever order the boxes were ticked in.
                sections: REPORT_SECTIONS.filter((s) => sections.includes(s.id)).map((s) => s.id),
              })
            : await call('export.segments', {
                ...common,
                ...(codeId ? { code_ids: [Number(codeId)] } : {}),
                ...(coderId && !finalLayer ? { coder_ids: [Number(coderId)] } : {}),
                ...(finalLayer ? { layer: 'final' } : {}),
              });
      setSaved(r);
      setPath(r.path);
      announce(t('Saved {path}.', { path: r.path }));
    } catch (err) {
      setError(toApiError(err).message);
    } finally {
      setBusy(false);
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save();
  };

  return (
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && close()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="export-title">
          <Heading slot="title" id="export-title" className="dialog-title">{t('Export')}</Heading>
          <NamesNotice what="file" />
          {error ? <Notice>{error}</Notice> : null}
          {saved ? <Notice kind="info">{t('Saved {path} ({size} KB).', { path: saved.path, size: num(Math.max(1, Math.round(saved.bytes / 1024))) })}</Notice> : null}
          {saved?.report ? <QdpxReportView report={saved.report} direction="out" /> : null}
          <form onSubmit={submit} noValidate>
            <RadioGroup value={what} onChange={(v) => changeWhat(v as What)} className="field">
              <Label>{t('What to export')}</Label>
              <Radio value="codebook">
                {rich('<span>The codebook</span><span2>Every code with its definition, criteria, and examples.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
              </Radio>
              <Radio value="segments">
                {rich('<span>The coded passages</span><span2>The words that were coded, with the code, the coder, and the source.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
              </Radio>
              <Radio value="project">
                {rich('<span>The whole project</span><span2>Everything the REFI-QDA exchange format holds, to open in another qualitative analysis program. It says what the format cannot hold.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
              </Radio>
              <Radio value="report">
                {rich('<span>A report</span><span2>A Word document made from the project records: the codebook, agreement, a draft methods appendix, and more.</span2>', { span: (inside) => <span className="radio-title">{inside}</span>, span2: (inside) => <span className="radio-help">{inside}</span> })}
              </Radio>
            </RadioGroup>

            {formats.length > 1 ? (
            <RadioGroup value={chosenFormat} onChange={(v) => setFormat(v as ExportFormat)} className="field">
              <Label>{t('File type')}</Label>
              {formats.map((f) => (
                <Radio key={f.id} value={f.id}>
                  <span className="radio-title">{f.label}</span>
                  <span className="radio-help">{f.help}</span>
                </Radio>
              ))}
            </RadioGroup>
            ) : null}

            {what === 'report' ? (
              <>
                <TextField value={title} onChange={setTitle} className="field">
                  <Label>{t('Title of the report')}</Label>
                  <Input placeholder={t('Report: {name}', { name: project?.name ?? '' })} data-testid="report-title" />
                  <Text slot="description">{t('Leave it empty to use the name of the project.')}</Text>
                </TextField>
                <fieldset className="field bk-picture-facts" data-testid="report-sections">
                  <legend>{t('What the report holds')}</legend>
                  {REPORT_SECTIONS.map((s) => (
                    <Tick key={s.id} isSelected={sections.includes(s.id)} onChange={(on) => setSections(on ? [...sections, s.id] : sections.filter((x) => x !== s.id))} testId={`report-section-${s.id}`}>
                      <span className="radio-title">{s.label}</span> <span className="radio-help">{s.help}</span>
                    </Tick>
                  ))}
                </fieldset>
              </>
            ) : what === 'codebook' ? (
              <Checkbox isSelected={inactive} onChange={setInactive} className="check">
                <div className="box" aria-hidden="true">
            <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
              <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
            </svg>
          </div>{t('Include merged and retired codes')}</Checkbox>
            ) : what === 'project' ? (
              <>
                <p className="help" data-testid="qdpx-note">{t('The documents go into the file whole, with any names in them. The file is not encrypted, even if the project is.')}</p>
                <Tick isSelected={withRecordings} onChange={setWithRecordings} testId="export-qdpx-recordings">
                  <span className="radio-title">{t('Put the recordings inside the file')}</span> <span className="radio-help">{t('Without this the file only says where each recording is on this computer. With it the recordings go in, which can make the file very large; a recording over 512 MB, or past 1 GB in all, is left out and named.')}</span>
                </Tick>
              </>
            ) : (
              <>
                <div className="field">
                  <label htmlFor="exp-code">{t('Only this code')}</label>
                  <select id="exp-code" value={codeId} onChange={(e) => setCodeId(e.target.value)}>
                    <option value="">{t('All codes')}</option>
                    {activeCodes.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                  <p className="help">{t('A code that has sub-codes includes theirs.')}</p>
                </div>
                <div className="field">
                  <label htmlFor="exp-coder">{t('Only this coder')}</label>
                  <select id="exp-coder" value={coderId} disabled={finalLayer} onChange={(e) => setCoderId(e.target.value)}>
                    <option value="">{t('Every coder')}</option>
                    {coders.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                        {c.role === 'model' ? ' (model)' : ''}
                      </option>
                    ))}
                  </select>
                </div>
                <Tick isSelected={finalLayer} onChange={setFinalLayer} testId="export-final-layer">
                  <span className="radio-title">{t('Use the final layer')}</span> <span className="radio-help">{t('What was decided in Team > Disagreements, and what the coders agreed on, instead of every coder’s own codings. Units, areas, and stretches the coders still disagree on, and nobody has decided, are left out.')}</span>
                </Tick>
              </>
            )}

            <TextField value={path} onChange={setPath} className="field">
              <Label>{t('Where to save it')}</Label>
              <Input placeholder={`C:\\Reports\\${suggestion}`} />
              <Text slot="description">{nativeDialogsAvailable ? t('Type the full path, or use Browse. The ending .{chosenFormat} is added if you leave it out. A file that is already there is not replaced.', { chosenFormat }) : t('Type the full path. The ending .{chosenFormat} is added if you leave it out. A file that is already there is not replaced.', { chosenFormat })}</Text>
            </TextField>
            <div className="actions">
              <Button type="submit" className="primary" isDisabled={busy}>
                {busy ? t('Saving…') : t('Save the file')}
              </Button>
              {nativeDialogsAvailable ? (
                <Button
                  type="button"
                  aria-label={t('Browse for where to save the file')}
                  isDisabled={busy}
                  onPress={async () => {
                    const picked = await browsePath('export-file', suggestion, chosenFormat);
                    if (picked) {
                      setPath(picked);
                      void save(picked, true);
                    }
                  }}
                >{t('Browse…')}</Button>
              ) : null}
              <Button type="button" onPress={close}>
                {saved ? t('Close') : t('Cancel')}
              </Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
