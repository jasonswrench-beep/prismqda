import { useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { ComboBox, Dialog, Heading, Input, Label, ListBox, ListBoxItem, Modal, ModalOverlay, Popover, type Key } from 'react-aria-components';
import { toApiError } from '../bridge';
import { abbreviations, nextColor } from '../lib/labels';
import { pathLabel } from '../lib/tree';
import { addCode, applyCode } from '../state/actions';
import { announce } from '../state/announce';
import { appStore, focusCenter, keptTextSelection, useApp } from '../state/app';
import { compareText, t } from '../i18n';

interface Option {
  id: string;
  name: string;
  kind: 'code' | 'new';
  codeId?: number;
}

const NEW_ID = '__new__';

/**
 * Code Selection: a small dialog with one box. Type to find a code, press Enter or choose it, and it is applied to the
 * selected words (or the unit at the caret in a project that codes whole units). Typing a name that is not in the
 * codebook offers to make that code and apply it. The text keeps its selection while this is open, and the focus
 * goes back to the text afterwards, so coding goes on from the keyboard.
 */
export function CodePickerDialog() {
  const codes = useApp((s) => s.codes);
  const selectionText = useApp((s) => s.selectionText);
  const [q, setQ] = useState('');
  const [selectedKey, setSelectedKey] = useState<Key | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The words that were selected when the dialog opened. Moving the focus here and back can collapse the selection in the
  // text, so it is given back explicitly when the dialog closes.
  const [kept] = useState(() => keptTextSelection());

  const labels = useMemo(() => abbreviations(codes), [codes]);
  const options = useMemo<Option[]>(() => {
    const needle = q.trim().toLowerCase();
    const active = codes.filter((c) => c.status === 'active');
    const hits = active
      .filter((c) => needle === '' || c.name.toLowerCase().includes(needle) || (labels.get(c.id) ?? '').toLowerCase() === needle)
      .sort((a, b) => {
        // Names that start with the typed letters come first.
        const as = needle !== '' && a.name.toLowerCase().startsWith(needle) ? 0 : 1;
        const bs = needle !== '' && b.name.toLowerCase().startsWith(needle) ? 0 : 1;
        return as - bs || compareText(a.name, b.name);
      })
      .slice(0, 50)
      .map<Option>((c) => ({ id: String(c.id), name: pathLabel(codes, c.id), kind: 'code', codeId: c.id }));
    const exact = active.some((c) => c.parent_id === null && c.name.toLowerCase() === needle);
    if (needle !== '' && !exact) hits.push({ id: NEW_ID, name: t('Make a new code named "{name}" and apply it', { name: q.trim() }), kind: 'new' });
    return hits;
  }, [codes, q, labels]);

  const close = () => {
    appStore.set({ dialog: null });
    // Back to the text, so coding can go on from the keyboard.
    window.setTimeout(() => focusCenter(kept), 0);
  };

  const choose = async (opt: Option | undefined) => {
    if (!opt) return;
    appStore.set({ dialog: null });
    try {
      if (opt.kind === 'new') {
        const code = await addCode({ name: q.trim(), parentId: null, color: nextColor(codes) });
        await applyCode(code.id);
      } else if (opt.codeId !== undefined) {
        await applyCode(opt.codeId);
      }
    } catch (e) {
      announce(toApiError(e).message, 'assertive');
    }
    window.setTimeout(() => focusCenter(kept), 0);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    // Enter with no highlighted choice applies the first match.
    if (e.key === 'Enter' && !e.currentTarget.getAttribute('aria-activedescendant') && options.length > 0 && q.trim() !== '') {
      e.preventDefault();
      e.stopPropagation();
      void choose(options[0]);
    }
  };

  return (
    <ModalOverlay isOpen isDismissable onOpenChange={(open) => !open && close()} className="overlay">
      <Modal className="modal bk-picker-modal">
        <Dialog className="dialog bk-picker" aria-labelledby="pick-code-title">
          <Heading slot="title" id="pick-code-title" className="dialog-title">{t('Apply a code')}</Heading>
          <p className="muted bk-picker-selection" data-testid="picker-selection">
            {selectionText || t('Nothing is selected, so the unit at the caret is coded if the project codes whole units.')}
          </p>
          <ComboBox
            className="field apply-box"
            items={options}
            inputValue={q}
            onInputChange={setQ}
            selectedKey={selectedKey}
            onSelectionChange={(key) => {
              setSelectedKey(key);
              if (key !== null) void choose(options.find((o) => o.id === String(key)));
            }}
            menuTrigger="focus"
            allowsEmptyCollection
            autoFocus
            aria-label={t('Apply a code')}
          >
            <Label className="visually-hidden">{t('Apply a code')}</Label>
            <div className="combo-row">
              <Input ref={inputRef} placeholder={t('Find a code to apply')} onKeyDown={onKeyDown} />
            </div>
            <Popover className="popover">
              <ListBox className="menu" renderEmptyState={() => <div className="menu-empty">{t('No active code matches. Type a name to make a new one.')}</div>}>
                {(item: Option) => (
                  <ListBoxItem id={item.id} textValue={item.name} className={`menu-item${item.kind === 'new' ? ' menu-new' : ''}`}>
                    {item.name}
                  </ListBoxItem>
                )}
              </ListBox>
            </Popover>
          </ComboBox>
          <p className="help">{t('Press Enter to apply the first match, Escape to cancel.')}</p>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
