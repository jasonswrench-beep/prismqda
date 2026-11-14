import { useState, type FormEvent } from 'react';
import { Button, Dialog, Heading, Input, Label, Modal, ModalOverlay, Radio, RadioGroup, Text, TextField } from 'react-aria-components';
import { call, toApiError } from '../bridge';
import { dateTime, num, t, tn } from '../i18n';
import { dataCsv, scriptText, tableKeys } from '../lib/rScript';
import { drawingToPng, drawSvg, notebookPayload } from '../lib/statExport';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../pickFile';
import { announce } from '../state/announce';
import { useApp } from '../state/app';
import { analyses, useStats } from '../state/statistics';
import type { NotebookFileFormat } from '../types';
import { Notice } from './common';

/** What is being saved: the whole notebook, or one chart (given by a function that finds its drawing on the page when it is saved). */
export type ExportTarget = { kind: 'notebook' } | { kind: 'chart'; svg: () => SVGSVGElement | null; name: string };

type Format = NotebookFileFormat | 'r' | 'png' | 'svg';

const NOTEBOOK_FORMATS: { id: NotebookFileFormat | 'r'; label: () => string; help: () => string }[] = [
  { id: 'docx', label: () => t('Word'), help: () => t('Numbered APA 7 tables and figures, each page with its results sentence and reading. Opens in Word and other word processors.') },
  { id: 'xlsx', label: () => t('Excel'), help: () => t('A sheet for each page, with a Contents sheet. A table value that is a plain number is a number in the cell. Charts are pictures.') },
  { id: 'html', label: () => t('Web page'), help: () => t('One file that opens in any browser, with the charts as drawings and a text alternative for each. To make a PDF, print it from the browser.') },
  { id: 'r', label: () => t('R script'), help: () => t('Every page as R code that prints the numbers the page shows, with the data it read saved beside it as a CSV file. Run it with R, in the folder that holds the two files.') },
];

const CHART_FORMATS: { id: 'png' | 'svg'; label: () => string; help: () => string }[] = [
  { id: 'png', label: () => 'PNG', help: () => t('A picture for a document, a slide, or a web page, drawn at twice the size it has on the page so it stays sharp.') },
  { id: 'svg', label: () => 'SVG', help: () => t('A drawing that stays sharp at any size, for a program that draws or lays out pages.') },
];

function safeName(text: string): string {
  const cleaned = text.replace(/[\\/:*?"<>|]+/g, '-').replace(/\s+/g, ' ').trim();
  return cleaned === '' ? 'PrismQDA' : cleaned.slice(0, 80);
}

export function StatExportDialog({ target, onClose }: { target: ExportTarget; onClose: () => void }) {
  const project = useApp((s) => s.project);
  const notebook = analyses(useStats((s) => s.notebook));
  const chart = target.kind === 'chart';
  const formats = chart ? CHART_FORMATS : NOTEBOOK_FORMATS;
  const [format, setFormat] = useState<Format>(chart ? 'png' : 'docx');
  const [title, setTitle] = useState('');
  const [path, setPath] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<{ path: string; bytes: number; data_path?: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const defaultTitle = t('Statistics: {name}', { name: project?.name ?? 'PrismQDA' });
  const ending = format === 'r' ? 'R' : format;
  const suggestion = `${safeName(chart ? target.name : title.trim() || defaultTitle)}.${ending}`;

  const save = async (typed?: string, fromDialog = false) => {
    setError(null);
    setSaved(null);
    const where = cleanTypedPath(typed ?? path);
    if (where === '') {
      const picked = await browsePath('export-file', suggestion, ending);
      if (!picked) return setError(t('Type where to save the file, for example C:\\Reports\\{file}', { file: suggestion }));
      return save(picked, true);
    }
    setBusy(true);
    try {
      // A path chosen in Windows' own save dialog has already been confirmed; a typed one never replaces a file.
      const overwrite = fromDialog;
      let r;
      if (target.kind === 'chart') {
        const svg = target.svg();
        if (!svg) throw new Error(t('The chart is not on the page, so it cannot be saved. Go back to the notebook and try again.'));
        const drawing = drawSvg(svg);
        r = await call('export.picture', { path: where, overwrite, format: format === 'svg' ? 'svg' : 'png', data: format === 'svg' ? drawing.text : await drawingToPng(drawing) });
      } else {
        if (notebook.length === 0) throw new Error(t('There is nothing in the notebook to export.'));
        if (format === 'r') {
          // The script is written from the pages as they ran and from the table of the cases' values at this moment.
          const variables = (await call('stats.variables', {})).variables;
          const { csv, classes } = dataCsv(await call('stats.table', { keys: tableKeys(notebook, variables) }));
          r = await call('export.script', { path: where, overwrite, script: scriptText(notebook, variables, classes, title.trim() || defaultTitle), data: csv });
          setSaved(r);
          setPath(r.path);
          announce(t('Saved {path}.', { path: r.path }));
          return;
        }
        const kind = format as NotebookFileFormat;
        const payload = await notebookPayload(notebook, {
          title: title.trim() || defaultTitle,
          pictures: kind !== 'html',
          drawings: kind === 'html',
          footer: t('Made with PrismQDA on {date}.', { date: dateTime(new Date(), { dateStyle: 'long' }) }),
        });
        r = await call('export.notebook', { path: where, overwrite, format: kind, notebook: payload });
      }
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
    <ModalOverlay isOpen isDismissable={false} onOpenChange={(o) => !o && onClose()} className="overlay">
      <Modal className="modal modal-wide">
        <Dialog className="dialog" aria-labelledby="stat-export-title" data-testid="stat-export-dialog">
          <Heading slot="title" id="stat-export-title" className="dialog-title">
            {chart ? t('Save the chart as a picture') : t('Export the notebook')}
          </Heading>
          {error ? <Notice>{error}</Notice> : null}
          {saved ? (
            <Notice kind="info">
              {saved.data_path
                ? t('Saved {path} and its data file {data}.', { path: saved.path, data: saved.data_path })
                : t('Saved {path} ({size} KB).', { path: saved.path, size: num(Math.max(1, Math.round(saved.bytes / 1024))) })}
            </Notice>
          ) : null}
          <form onSubmit={submit} noValidate>
            {chart ? null : (
              <p className="muted" data-testid="stat-export-count">
                {tn(notebook.length, { one: '{count} page is in the notebook, and it is exported.', other: '{count} pages are in the notebook, and all of them are exported.' })}
              </p>
            )}
            <RadioGroup
              value={format}
              onChange={(v) => {
                setFormat(v as Format);
                // The name of a file saved a moment ago ends in the old kind; the engine adds the ending of the new kind.
                setPath((was) => was.replace(/\.(docx|xlsx|html|png|svg|r)$/i, ''));
              }}
              className="field"
            >
              <Label>{t('File type')}</Label>
              {formats.map((f) => (
                <Radio key={f.id} value={f.id} data-testid={`stat-export-${f.id}`}>
                  <span className="radio-title">{f.label()}</span>
                  <span className="radio-help">{f.help()}</span>
                </Radio>
              ))}
            </RadioGroup>
            {chart ? null : (
              <TextField value={title} onChange={setTitle} className="field">
                <Label>{t('Title')}</Label>
                <Input placeholder={defaultTitle} data-testid="stat-export-title" />
                <Text slot="description">{t('Leave it empty to use the name of the project.')}</Text>
              </TextField>
            )}
            <TextField value={path} onChange={setPath} className="field">
              <Label>{t('Where to save it')}</Label>
              <Input placeholder={`C:\\Reports\\${suggestion}`} data-testid="stat-export-path" />
              <Text slot="description">
                {nativeDialogsAvailable
                  ? t('Type the full path, or use Browse. The ending .{format} is added if you leave it out. A file that is already there is not replaced.', { format: ending })
                  : t('Type the full path. The ending .{format} is added if you leave it out. A file that is already there is not replaced.', { format: ending })}
              </Text>
            </TextField>
            <div className="actions">
              <Button type="submit" className="primary" isDisabled={busy} data-testid="stat-export-save">
                {busy ? t('Saving…') : t('Save the file')}
              </Button>
              {nativeDialogsAvailable ? (
                <Button
                  type="button"
                  aria-label={t('Browse for where to save the file')}
                  isDisabled={busy}
                  onPress={async () => {
                    const picked = await browsePath('export-file', suggestion, ending);
                    if (picked) {
                      setPath(picked);
                      void save(picked, true);
                    }
                  }}
                >
                  {t('Browse…')}
                </Button>
              ) : null}
              <Button type="button" onPress={onClose} data-testid="stat-export-close">
                {saved ? t('Close') : t('Cancel')}
              </Button>
            </div>
          </form>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
