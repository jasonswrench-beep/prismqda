// The working set in the running program (decision 163): kept in the app state, applied to what is asked of the engine, and forgotten when
// the project is closed or another is opened. The rules themselves are in lib/workingSet.ts.

import { ApiError, setArgsTransform } from '../bridge';
import { applyWorkingSet, EVERYTHING, narrows, pruneWorkingSet, WorkingSetEmpty, type WorkingSet } from '../lib/workingSet';
import { announce } from './announce';
import { appStore } from './app';
import { t } from '../i18n';

/** The working set with whatever has gone from the project taken out. */
export function currentWorkingSet(): WorkingSet {
  const s = appStore.get();
  if (!narrows(s.workingSet)) return s.workingSet;
  return pruneWorkingSet(s.workingSet, { sources: s.sources.map((x) => x.id), cases: s.cases.map((c) => c.id), codes: s.codes.filter((c) => c.status === 'active').map((c) => c.id) });
}

/** From here on every request goes to the engine through the working set. */
setArgsTransform((cmd, args) => {
  try {
    return applyWorkingSet(cmd, args, currentWorkingSet(), appStore.get().cases);
  } catch (e) {
    if (e instanceof WorkingSetEmpty) throw new ApiError('invalid', t('The working set leaves no code to ask about here. Change the working set, or show everything.'));
    throw e;
  }
});

/** Chooses what to work with. A set that narrows nothing is the same as none. */
export function setWorkingSet(w: WorkingSet): void {
  // The version moves so that every page that reloads when the data changes asks again, through the new set.
  appStore.set((s) => ({ workingSet: narrows(w) ? w : EVERYTHING, dataVersion: s.dataVersion + 1 }));
  announce(narrows(w) ? t('Now working with a part of the project. Every page that can follow the working set does.') : t('Now working with the whole project.'));
}

/** Back to the whole project. */
export function resetWorkingSet(): void {
  appStore.set((s) => ({ workingSet: EVERYTHING, dataVersion: s.dataVersion + 1 }));
  announce(t('Now working with the whole project.'));
}
