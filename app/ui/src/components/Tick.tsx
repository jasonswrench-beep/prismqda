import type { ReactNode } from 'react';
import { Checkbox } from 'react-aria-components';

/** A check box in the program's own look: a box that shows a tick, then the words that say what it is for. */
export function Tick(props: { isSelected: boolean; onChange: (on: boolean) => void; children: ReactNode; testId?: string; isDisabled?: boolean }) {
  return (
    <Checkbox isSelected={props.isSelected} onChange={props.onChange} className="check" data-testid={props.testId} isDisabled={props.isDisabled}>
      <div className="box" aria-hidden="true">
        <svg viewBox="0 0 18 18" width="14" height="14" focusable="false">
          <polyline points="1 9 7 14 15 4" fill="none" stroke="currentColor" strokeWidth="2.4" />
        </svg>
      </div>
      {props.children}
    </Checkbox>
  );
}
