// The Documents pane as a tree: the project at the top, its folders beneath it, and the documents in them. A folder's
// name in the project is a path ("Interviews", or "Interviews/Spring" for a subfolder), so the tree follows it.

import type { SourceRow } from '../types';
import { compareText } from '../i18n';

export interface DocNode {
  /** "root", "f:Interviews/Spring" for a folder, "d12" for the document with id 12. */
  key: string;
  kind: 'root' | 'folder' | 'document';
  label: string;
  /** Documents at or beneath this node (1 for a document). */
  count: number;
  /** The source, for a document. */
  source?: SourceRow;
  children: DocNode[];
}

/** The parts of a folder name. Both kinds of slash separate parts; empty parts are ignored. */
export function folderParts(folder: string): string[] {
  return folder
    .split(/[\\/]+/)
    .map((p) => p.trim())
    .filter(Boolean);
}

function compareNames(a: string, b: string): number {
  return compareText(a, b, { numeric: true, sensitivity: 'base' });
}

/** Builds the tree. Folders come first, in name order, then the documents of that level in name order. */
export function buildDocTree(projectName: string, sources: readonly SourceRow[]): DocNode {
  const root: DocNode = { key: 'root', kind: 'root', label: projectName, count: 0, children: [] };
  const folders = new Map<string, DocNode>();
  const folderAt = (parts: string[]): DocNode => {
    let parent = root;
    let path = '';
    for (const part of parts) {
      path = path === '' ? part : `${path}/${part}`;
      let node = folders.get(path);
      if (!node) {
        node = { key: `f:${path}`, kind: 'folder', label: part, count: 0, children: [] };
        folders.set(path, node);
        parent.children.push(node);
      }
      parent = node;
    }
    return parent;
  };
  for (const s of sources) {
    const home = folderAt(folderParts(s.folder));
    home.children.push({ key: `d${s.id}`, kind: 'document', label: s.name, count: 1, source: s, children: [] });
  }
  const finish = (node: DocNode): number => {
    for (const child of node.children) if (child.kind !== 'document') finish(child);
    node.children.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1;
      return compareNames(a.label, b.label);
    });
    node.count = node.children.reduce((sum, c) => sum + c.count, 0);
    return node.count;
  };
  finish(root);
  return root;
}

/**
 * The tree narrowed to the documents whose names contain what was typed (words in any order, case ignored), with the
 * folders that hold them kept so each match shows where it lives. Counts become counts of the matches. An empty search
 * returns the tree as it is.
 */
export function filterDocTree(root: DocNode, query: string): DocNode {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return root;
  const walk = (node: DocNode): DocNode | null => {
    if (node.kind === 'document') {
      const hay = node.label.toLowerCase();
      return words.every((w) => hay.includes(w)) ? node : null;
    }
    const children = node.children.map(walk).filter((c): c is DocNode => c !== null);
    if (children.length === 0 && node.kind !== 'root') return null;
    return { ...node, children, count: children.reduce((sum, c) => sum + c.count, 0) };
  };
  return walk(root) ?? { ...root, children: [], count: 0 };
}

/** The keys of every folder (and the root) in a tree, for expanding or collapsing them all. */
export function folderKeys(node: DocNode, out: string[] = []): string[] {
  if (node.kind !== 'document') {
    out.push(node.key);
    for (const c of node.children) folderKeys(c, out);
  }
  return out;
}

/** The folders that hold a document, from the top down, so the tree can open them to show it. */
export function ancestorKeys(root: DocNode, docKey: string): string[] {
  const path: string[] = [];
  const find = (node: DocNode): boolean => {
    if (node.key === docKey) return true;
    for (const c of node.children) {
      if (find(c)) {
        if (node.kind !== 'document') path.unshift(node.key);
        return true;
      }
    }
    return false;
  };
  find(root);
  return path;
}
