import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { toApiError } from '../bridge';
import { useApp } from '../state/app';
import type { CaseVariable } from '../types';
import type { Choice } from './explore/shared';

/** The most different values a variable may have to be offered a column, a group, or an axis for each. */
export const MOST_CATEGORIES = 12;

export const SCALE = ['interval', 'ratio'];

/**
 * What was set up, kept while the project is open: opening a quotation in its document leaves this page, and coming back should find the
 * display as it was. A different project starts afresh.
 */
const kept: { path: string | null; values: Record<string, unknown> } = { path: null, values: {} };

export function useKept<T>(key: string, initial: T): [T, Dispatch<SetStateAction<T>>] {
  const path = useApp((s) => s.project?.path ?? null);
  if (kept.path !== path) {
    kept.path = path;
    kept.values = {};
  }
  const [value, setValue] = useState<T>(() => (key in kept.values ? (kept.values[key] as T) : initial));
  useEffect(() => {
    kept.values[key] = value;
  }, [key, value]);
  return [value, setValue];
}

/** What the variables of cases offer a display: a name for the picker, and the ones a column of categories can hold. */
export function variableChoices(vars: CaseVariable[], filter: (v: CaseVariable) => boolean): { list: CaseVariable[]; choices: Choice[] } {
  const list = vars.filter(filter);
  return { list, choices: list.map((v, i) => ({ id: i, label: v.name })) };
}

/** Works a display out whenever the question or the project changes, and keeps only the answer to the latest question. */
export function useDisplay<R>(spec: object | null, run: () => Promise<R>): { result: R | null; error: string | null; loading: boolean } {
  const version = useApp((s) => s.dataVersion);
  const codings = useApp((s) => s.sources.reduce((n, x) => n + x.codings, 0));
  const key = JSON.stringify(spec);
  const [state, setState] = useState<{ result: R | null; error: string | null; loading: boolean }>({ result: null, error: null, loading: false });
  useEffect(() => {
    if (spec === null) {
      setState({ result: null, error: null, loading: false });
      return;
    }
    let live = true;
    setState((s) => ({ ...s, loading: true, error: null }));
    run()
      .then((result) => live && setState({ result, error: null, loading: false }))
      .catch((e) => live && setState({ result: null, error: toApiError(e).message, loading: false }));
    return () => {
      live = false;
    };
    // `key` is the question; the display is also worked out again when the project changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, codings]);
  return state;
}
