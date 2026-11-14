import { useState } from 'react';
import { Button, Input, Label, Radio, RadioGroup, Text, TextField, TextArea } from 'react-aria-components';
import { call, toApiError } from '../../bridge';
import { browsePath, cleanTypedPath, nativeDialogsAvailable } from '../../pickFile';
import { announce } from '../../state/announce';
import { useApp } from '../../state/app';
import type { CoderPacketReport } from '../../types';
import { Notice } from '../common';
import { Tick } from '../Tick';
import { tn, t } from '../../i18n';

/** Where a packet goes when none is typed: beside the project, named for it and the coder. */
export function suggestedPacketPath(projectPath: string, coder: string): string {
  const slash = Math.max(projectPath.lastIndexOf('\\'), projectPath.lastIndexOf('/'));
  const folder = slash >= 0 ? projectPath.slice(0, slash + 1) : '';
  const stem = projectPath.slice(slash + 1).replace(/\.pqda$/i, '');
  const who = coder.trim().replace(/[\\/:*?"<>|]/g, '').replace(/\s+/g, ' ');
  return folder + t('{stem} - packet for {who}.pqda', { stem, who: who || t('coder') });
}

/**
 * Packets: a small project made for another coder (decision 161). It holds the codebook, the documents chosen, and the instructions, and
 * nothing else of the study. The coder opens it in PrismQDA, codes, and sends the file back; Merge brings their coding in, kept with them.
 */
export function PacketsTab() {
  const project = useApp((s) => s.project);
  const sources = useApp((s) => s.sources);
  const readOnly = project?.read_only ?? false;
  const [coder, setCoder] = useState('');
  const [which, setWhich] = useState<'all' | 'some'>('all');
  const [picked, setPicked] = useState<number[]>([]);
  const [instructions, setInstructions] = useState('');
  const [withCodings, setWithCodings] = useState(false);
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<CoderPacketReport | null>(null);

  const fallback = suggestedPacketPath(project?.path ?? '', coder);

  const make = async () => {
    setError(null);
    setDone(null);
    if (coder.trim() === '') return setError(t('Say who the packet is for.'));
    if (which === 'some' && picked.length === 0) return setError(t('Choose at least one document, or choose all of them.'));
    setBusy(true);
    try {
      const r = await call('packet.make', {
        path: cleanTypedPath(path) || fallback,
        coder_name: coder,
        source_ids: which === 'some' ? picked : [],
        instructions,
        include_codings: withCodings,
      });
      setDone(r);
      announce(t('The packet for {coder} was made at {path}.', { coder: r.coder, path: r.path }));
    } catch (e) {
      setError(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="ac-card" aria-labelledby="h-team-packets" data-testid="packets">
      <h3 id="h-team-packets" className="bk-h3">{t('Make a packet for another coder')}</h3>
      <p className="help">{t('A packet is a small project file for one coder: the codebook, the documents you choose, and your instructions, and nothing else of the study. They open it in PrismQDA, code, and send the file back. Bring it in on the Merge tab, and their coding is kept with them. The packet is not encrypted.')}</p>
      {error ? <Notice>{error}</Notice> : null}
      {readOnly ? <Notice kind="warning">{t('This project is read-only, but a packet can still be made from it.')}</Notice> : null}
      <TextField value={coder} onChange={(v) => { setCoder(v); setDone(null); }} className="field">
        <Label>{t('The coder the packet is for')}</Label>
        <Input data-testid="packet-coder" autoComplete="off" />
        <Text slot="description">{t('A coder who is already in this project with the same name is the same coder in the packet, so their work comes back to them.')}</Text>
      </TextField>

      <RadioGroup value={which} onChange={(v) => setWhich(v as 'all' | 'some')} className="field ac-radios" aria-label={t('Which documents')}>
        <Radio value="all" className="react-aria-Radio ac-radio" data-testid="packet-all">
          <span className="ac-radio-label">{t('All the documents')}</span>
        </Radio>
        <Radio value="some" className="react-aria-Radio ac-radio" data-testid="packet-some">
          <span className="ac-radio-label">{t('Only the documents I choose')}</span>
        </Radio>
      </RadioGroup>
      {which === 'some' ? (
        <ul className="packet-docs" aria-label={t('Documents to put in the packet')} data-testid="packet-docs">
          {sources.map((s) => (
            <li key={s.id}>
              <Tick isSelected={picked.includes(s.id)} onChange={(on) => setPicked(on ? [...picked, s.id] : picked.filter((x) => x !== s.id))} testId={`packet-doc-${s.id}`}>{s.name}</Tick>
            </li>
          ))}
        </ul>
      ) : null}

      <TextField value={instructions} onChange={setInstructions} className="field">
        <Label>{t('Instructions for the coder')}</Label>
        <TextArea rows={4} data-testid="packet-instructions" />
        <Text slot="description">{t('They see this as a memo in the packet: which codes to use, what to do with unclear passages, when to send it back.')}</Text>
      </TextField>
      <Tick isSelected={withCodings} onChange={setWithCodings} testId="packet-with-codings">{t('Also include the codings made so far, for the coder to review (otherwise they code blind)')}</Tick>

      <TextField value={path} onChange={setPath} className="field">
        <Label>{t('Save the packet as')}</Label>
        <div className="bk-location-row">
          <Input placeholder={fallback} data-testid="packet-path" />
          {nativeDialogsAvailable ? (
            <Button
              type="button"
              aria-label={t('Browse for where to save the packet')}
              isDisabled={busy}
              onPress={async () => {
                const chosen = await browsePath('new-project', fallback.split(/[\\/]/).pop()?.replace(/\.pqda$/i, '') ?? '');
                if (chosen) setPath(chosen);
              }}
            >{t('Browse…')}</Button>
          ) : null}
        </div>
        <Text slot="description">{t('Leave it empty to save it beside this project. A file that is already there is not replaced.')}</Text>
      </TextField>
      <div className="actions">
        <Button className="primary" onPress={() => void make()} isDisabled={busy} data-testid="packet-make">{busy ? t('Making…') : t('Make the packet')}</Button>
      </div>

      {done ? (
        <div data-testid="packet-result">
          <Notice kind="info">{t('The packet for {coder} was made at {path}.', { coder: done.coder, path: done.path })}</Notice>
          <ul data-testid="packet-sentences">
            <li>{tn(done.documents.length, { one: '{count} document: {names}', other: '{count} documents: {names}' }, { names: done.documents.join(', ') })}</li>
            <li>{tn(done.codes, { one: '{count} code in the codebook', other: '{count} codes in the codebook' })}</li>
            {done.codings > 0 ? <li>{tn(done.codings, { one: '{count} coding, for the coder to review', other: '{count} codings, for the coder to review' })}</li> : null}
            <li>{done.coder_known ? t('The coder is already in this project, so their work will come back to them.') : t('The coder is new to this project; Merge adds them when their work comes back.')}</li>
          </ul>
          <h4 className="bk-h3">{t('What the packet does not hold')}</h4>
          <ul data-testid="packet-left-out">
            {done.left_out.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
