// Keyboard shortcuts: what each key does, in one table that both the key handlers and the Help
// sheet read, so the printed sheet can never disagree with the program.
import { t } from '../i18n';

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  isComposing?: boolean;
}

/** What a key press in the text or the grid asks for. */
export type DocCommand =
  | { type: 'favorite'; slot: number }
  | { type: 'pick' }
  | { type: 'invivo' }
  | { type: 'repeat' }
  | { type: 'next' }
  | { type: 'previous' }
  | { type: 'examine' }
  | { type: 'selectUnit' };

/**
 * Single keys only work while the text (or the grid) has the focus, so they never interfere with
 * typing in a box and a screen reader's own keys keep working elsewhere.
 */
export function interpretDocumentKey(e: KeyLike): DocCommand | null {
  if (e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return null;
  const k = e.key;
  if (/^[1-9]$/.test(k) && !e.shiftKey) return { type: 'favorite', slot: Number(k) };
  if (k === 'Enter' && !e.shiftKey) return { type: 'pick' };
  const lower = k.length === 1 ? k.toLowerCase() : k;
  if (lower === 'n') return e.shiftKey ? { type: 'previous' } : { type: 'next' };
  if (e.shiftKey) return null;
  if (lower === 'i') return { type: 'invivo' };
  if (lower === 'r') return { type: 'repeat' };
  if (lower === 'e') return { type: 'examine' };
  if (lower === 'u') return { type: 'selectUnit' };
  return null;
}

export type ReorganizeKey = 'up' | 'down' | 'indent' | 'outdent';

/** Alt with an arrow key reorganizes the selected code in the codebook. */
export function interpretCodebookKey(e: KeyLike): ReorganizeKey | null {
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return null;
  switch (e.key) {
    case 'ArrowUp':
      return 'up';
    case 'ArrowDown':
      return 'down';
    case 'ArrowRight':
      return 'indent';
    case 'ArrowLeft':
      return 'outdent';
    default:
      return null;
  }
}

/** The keys that move between the main areas of the screen. */
export function interpretAreaKey(e: KeyLike): 'next' | 'previous' | 'help' | null {
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.key === 'F6') return e.shiftKey ? 'previous' : 'next';
  if (e.key === 'F1') return 'help';
  return null;
}

/** What the window needs to know of the control that has the focus when Ctrl+Z is pressed. */
export interface FocusLike {
  tagName: string;
  isContentEditable: boolean;
  getAttribute(name: string): string | null;
}

/**
 * Whether Ctrl+Z belongs to the control with the focus (it undoes typing in a box) rather than to the project. The document's
 * text is an editable area that is read-only (aria-readonly), so it has no typing to undo: there Ctrl+Z undoes the project's
 * last change, as it does anywhere else.
 */
export function undoesTyping(el: FocusLike | null): boolean {
  if (!el) return false;
  if (/^(input|textarea|select)$/i.test(el.tagName)) return true;
  return el.isContentEditable && el.getAttribute('aria-readonly') !== 'true';
}

export interface ShortcutEntry {
  keys: string;
  does: string;
}

export interface ShortcutSection {
  title: string;
  note?: string;
  entries: ShortcutEntry[];
}

export const SHORTCUT_SECTIONS: ShortcutSection[] = [
  {
    get title() { return t('Anywhere'); },
    entries: [
      { get keys() { return t('Tab and Shift+Tab'); }, get does() { return t('Move to the next or previous control'); } },
      { get keys() { return t('F6 and Shift+F6'); }, get does() { return t('Jump to the next or previous area: Sources, Document, Codebook, Details'); } },
      { keys: 'F1', get does() { return t('Open this list of shortcuts'); } },
      { get keys() { return t('Escape'); }, get does() { return t('Close a window or a menu'); } },
    ],
  },
  {
    get title() { return t('In the document text'); },
    get note() { return t('These single keys work only while the text has the focus. Move the caret with the arrow keys; hold Shift to select.'); },
    entries: [
      { get keys() { return t('Arrow keys'); }, get does() { return t('Move the caret by letter or line'); } },
      { get keys() { return t('Ctrl+Left and Ctrl+Right'); }, get does() { return t('Move the caret by word'); } },
      { get keys() { return t('Shift+Arrow keys'); }, get does() { return t('Select text as you move the caret'); } },
      { get keys() { return t('Ctrl+Shift+Left and Ctrl+Shift+Right'); }, get does() { return t('Select by word'); } },
      { get keys() { return t('Home and End'); }, get does() { return t('Go to the start or end of the line (add Shift to select)'); } },
      { get keys() { return t('1 to 9'); }, get does() { return t('Apply the favorite code with that number to the selection. In a unit project with nothing selected, it codes the whole unit'); } },
      { get keys() { return t('Enter'); }, get does() { return t('Move to the "Apply a code" box to find a code by typing part of its name'); } },
      { keys: 'I', get does() { return t('In-vivo coding: make a code named from the selected words and apply it'); } },
      { keys: 'R', get does() { return t('Repeat the last code you applied'); } },
      { keys: 'U', get does() { return t('Select the whole unit at the caret'); } },
      { get keys() { return t('N and Shift+N'); }, get does() { return t('Go to the next or previous unit that has no coding'); } },
      { keys: 'E', get does() { return t('Open the coding at the caret in the Details area to edit it'); } },
      { get keys() { return t('Escape'); }, get does() { return t('Put away the small bar that floats above words selected with the mouse; the words stay selected'); } },
    ],
  },
  {
    get title() { return t('In the recording'); },
    get note() { return t('These keys work while the focus is on the player or on its buttons, and not in a box where you type a time.'); },
    entries: [
      { keys: 'I', get does() { return t('Mark where a stretch starts, at the time the recording is at'); } },
      { keys: 'O', get does() { return t('Mark where a stretch ends. Then choose a code to apply it to the stretch'); } },
    ],
  },
  {
    get title() { return t('In the grid of units'); },
    get note() { return t('The Grid and Thread views list one unit per row.'); },
    entries: [
      { get keys() { return t('Up and Down'); }, get does() { return t('Move to the previous or next row'); } },
      { get keys() { return t('Page Up, Page Down, Home, End'); }, get does() { return t('Move by a page or to the first or last row'); } },
      { get keys() { return t('Shift+Up and Shift+Down'); }, get does() { return t('Select several rows'); } },
      { get keys() { return t('1 to 9, R, N, Shift+N, E'); }, get does() { return t('The same as in the text, applied to the selected rows'); } },
    ],
  },
  {
    get title() { return t('In the codebook'); },
    entries: [
      { get keys() { return t('Up, Down, Left, Right'); }, get does() { return t('Move through the codes; Right opens a branch and Left closes it'); } },
      { get keys() { return t('Enter or double-click'); }, get does() { return t('Apply the selected code to the selected text'); } },
      { get keys() { return t('Alt+Up and Alt+Down'); }, get does() { return t('Move the selected code up or down among its neighbors'); } },
      { get keys() { return t('Alt+Right'); }, get does() { return t('Make the selected code a sub-code of the code above it'); } },
      { get keys() { return t('Alt+Left'); }, get does() { return t('Move the selected code out one level'); } },
      { get keys() { return t('Type letters'); }, get does() { return t('Jump to a code whose name starts with them (or use the "Find a code" box)'); } },
    ],
  },
  {
    get title() { return t('Dividing lines between areas'); },
    entries: [
      { get keys() { return t('Left and Right (or Up and Down)'); }, get does() { return t('Move a dividing line when it has the focus'); } },
      { get keys() { return t('Shift+Arrow keys'); }, get does() { return t('Move it in bigger steps'); } },
      { get keys() { return t('Home and End'); }, get does() { return t('Make the area as small or as large as it can be'); } },
    ],
  },
];

/** The favorite slot a number key stands for, as an index into the favorites list. */
export function slotIndex(slot: number): number {
  return slot - 1;
}
