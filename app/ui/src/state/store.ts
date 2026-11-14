// A very small store: one object of state, subscribers, and a hook that reads a slice of it.

import { useRef, useSyncExternalStore } from 'react';

export interface Store<S> {
  get(): S;
  set(patch: Partial<S> | ((s: S) => Partial<S>)): void;
  subscribe(listener: () => void): () => void;
}

export function createStore<S extends object>(initial: S): Store<S> {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => state,
    set(patch) {
      const part = typeof patch === 'function' ? patch(state) : patch;
      let changed = false;
      for (const k of Object.keys(part) as (keyof S)[]) {
        if (!Object.is(state[k], part[k])) {
          changed = true;
          break;
        }
      }
      if (!changed) return;
      state = { ...state, ...part };
      listeners.forEach((l) => l());
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** Reads one value from a store. The selector must return the same value when nothing relevant changed. */
export function useStore<S, T>(store: Store<S>, selector: (s: S) => T): T {
  const selectorRef = useRef(selector);
  selectorRef.current = selector;
  return useSyncExternalStore(
    store.subscribe,
    () => selectorRef.current(store.get()),
    () => selectorRef.current(store.get()),
  );
}
