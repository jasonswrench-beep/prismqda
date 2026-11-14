// Choosing the code a run will suggest: one of the codes in the codebook, or a new one named here.

import { useMemo } from 'react';
import { useApp } from '../../state/app';
import { codeChoices } from '../explore/shared';
import { t } from '../../i18n';

export interface CodePick {
  mode: 'existing' | 'new';
  codeId: number | null;
  newCode: string;
}

export function CodeSelect(props: {
  id: string;
  label: string;
  /** Hide the label visually (a table column says it); it stays for screen readers. */
  quietLabel?: boolean;
  value: CodePick;
  onChange: (next: CodePick) => void;
}) {
  const codes = useApp((s) => s.codes);
  const choices = useMemo(() => codeChoices(codes), [codes]);
  const { value } = props;
  const typed = value.newCode.trim().toLowerCase();
  const sameName = value.mode === 'new' && typed !== '' ? codes.find((c) => c.status === 'active' && c.name.toLowerCase() === typed) : undefined;
  return (
    <div className="field ac-code-select">
      <label htmlFor={props.id} className={props.quietLabel ? 'visually-hidden' : undefined}>
        {props.label}
      </label>
      <select
        id={props.id}
        value={value.mode === 'new' ? 'new' : value.codeId === null ? '' : String(value.codeId)}
        onChange={(e) => {
          const v = e.target.value;
          if (v === 'new') props.onChange({ ...value, mode: 'new', codeId: null });
          else props.onChange({ ...value, mode: 'existing', codeId: v === '' ? null : Number(v) });
        }}
      >
        <option value="">{t('Choose a code…')}</option>
        {choices.map((c) => (
          <option key={c.id} value={c.id}>
            {'  '.repeat(c.depth ?? 0)}
            {c.label}
          </option>
        ))}
        <option value="new">{t('A new code…')}</option>
      </select>
      {value.mode === 'new' ? (
        <>
          <label htmlFor={`${props.id}-name`} className="visually-hidden">{t('Name of the new code for {label}', { label: props.label })}
          </label>
          <input id={`${props.id}-name`} type="text" value={value.newCode} placeholder={t('Name of the new code')} onChange={(e) => props.onChange({ ...value, newCode: e.target.value })} />
          {sameName ? <span className="help ac-same-name">{t('A code named {name} already exists, so it will be used.', { name: sameName.name })}</span> : null}
        </>
      ) : null}
    </div>
  );
}
