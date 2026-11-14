import { useApp } from '../state/app';

/**
 * What is selected, in words, under the text: only while something is. With nothing selected the line is not drawn at
 * all, so the reader stays clean; a screen reader hears it as a polite status when a selection is made.
 */
export function SelectionLine() {
  const selectionText = useApp((s) => s.selectionText);
  return (
    <p className={`selection-line bk-selection${selectionText ? '' : ' empty'}`} data-testid="selection-line" role="status" aria-live="polite">
      {selectionText}
    </p>
  );
}
