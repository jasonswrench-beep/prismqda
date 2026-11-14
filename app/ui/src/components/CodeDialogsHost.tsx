import type { DialogState } from '../state/app';
import { CodebookCompareDialog } from './CodebookCompareDialog';
import { CounterexampleDialog, MergeDialog, NewCodeDialog, ReplaceExclusiveDialog, SnapshotDialog, SplitDialog } from './CodeDialogs';

/** Shows whichever codebook dialog is open. */
export function CodeDialogs({ dialog }: { dialog: DialogState | null }) {
  if (!dialog) return null;
  switch (dialog.kind) {
    case 'new-code':
      return <NewCodeDialog parentId={dialog.parentId} />;
    case 'merge':
      return <MergeDialog codeId={dialog.codeId} />;
    case 'split':
      return <SplitDialog codeId={dialog.codeId} />;
    case 'snapshot':
      return <SnapshotDialog />;
    case 'codebook-compare':
      return <CodebookCompareDialog />;
    case 'replace-exclusive':
      return <ReplaceExclusiveDialog pending={dialog.pending} />;
    case 'counterexample':
      return <CounterexampleDialog codeId={dialog.codeId} />;
    default:
      return null;
  }
}
