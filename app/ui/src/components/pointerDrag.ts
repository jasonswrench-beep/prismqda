import type { PointerEvent as ReactPointerEvent } from 'react';

// Dragging with the pointer, for moves inside the window.
//
// The browser's own drag and drop (HTML5) does not work in the Windows window: Tauri takes over the drop there so that files can be dropped
// from Explorer onto Import, and the page never sees an in-page drop. A drag made of pointer events works the same in every window. It is a
// quicker way for the mouse and the pen only; every move that can be dragged can also be made from a list, which is the way for the keyboard
// and for a screen reader.

/** How far the pointer must move, in pixels, before a press becomes a drag (so that a click on a row is not a drag). */
export const DRAG_THRESHOLD = 6;

/**
 * The controls inside a draggable row that keep their own press. A tick's label is not one: its words are the row's name, and a press that
 * does not move is still a click on it, which ticks it.
 */
const CONTROLS = 'button, a, select, input, textarea, [role="button"]';

export interface PointerDragOptions {
  /** The words shown beside the pointer while dragging. */
  label: string;
  /** The attribute that marks a place something can be dropped; its value says where. */
  target: string;
  /**
   * What `data-drop-over` says about the place under the pointer (it is `true` when this is left out). A place that is a row of a list can say
   * where in it the pointer is: before, on, or after.
   */
  mark?: (place: HTMLElement, event: PointerEvent) => string;
  /** Called with the value of the target attribute where the pointer was let go, and what the mark said there. Not called when it was let go anywhere else. */
  onDrop: (where: string, mark: string) => void;
}

/** True once the pointer has moved far enough from where it was pressed to be a drag. */
export function isDrag(dx: number, dy: number): boolean {
  return Math.hypot(dx, dy) >= DRAG_THRESHOLD;
}

/**
 * Call from `onPointerDown` of the row to drag. A press on a control inside the row, or with a button other than the main one, or by touch,
 * does nothing. While dragging, the place under the pointer that carries the target attribute is marked with `data-drop-over="true"`; Escape
 * stops the drag.
 */
export function beginPointerDrag(e: ReactPointerEvent<HTMLElement>, options: PointerDragOptions): void {
  if (e.button !== 0 || (e.pointerType !== 'mouse' && e.pointerType !== 'pen')) return;
  if ((e.target as HTMLElement).closest(CONTROLS)) return;
  const startX = e.clientX;
  const startY = e.clientY;
  let dragging = false;
  let ghost: HTMLElement | null = null;
  let over: HTMLElement | null = null;
  let overMark = 'true';

  const targetAt = (x: number, y: number): HTMLElement | null => {
    const el = document.elementFromPoint(x, y);
    return el instanceof HTMLElement ? el.closest<HTMLElement>(`[${options.target}]`) : null;
  };
  const mark = (el: HTMLElement | null, ev?: PointerEvent) => {
    const value = el && ev && options.mark ? options.mark(el, ev) : 'true';
    if (el === over && value === overMark) return;
    if (el !== over) over?.removeAttribute('data-drop-over');
    over = el;
    overMark = value;
    over?.setAttribute('data-drop-over', value);
  };
  const stop = () => {
    document.removeEventListener('pointermove', move);
    document.removeEventListener('pointerup', up);
    document.removeEventListener('pointercancel', cancel);
    document.removeEventListener('keydown', key, true);
    mark(null);
    ghost?.remove();
    ghost = null;
    document.body.classList.remove('dragging-pointer');
  };
  function move(ev: PointerEvent) {
    if (!dragging) {
      if (!isDrag(ev.clientX - startX, ev.clientY - startY)) return;
      dragging = true;
      ghost = document.createElement('div');
      ghost.className = 'drag-ghost';
      ghost.setAttribute('aria-hidden', 'true');
      ghost.textContent = options.label;
      document.body.appendChild(ghost);
      document.body.classList.add('dragging-pointer');
    }
    if (ghost) {
      ghost.style.left = `${ev.clientX + 12}px`;
      ghost.style.top = `${ev.clientY + 12}px`;
    }
    mark(targetAt(ev.clientX, ev.clientY), ev);
  }
  function up(ev: PointerEvent) {
    const place = dragging ? targetAt(ev.clientX, ev.clientY) : null;
    const where = place?.getAttribute(options.target) ?? null;
    const how = place && options.mark ? options.mark(place, ev) : 'true';
    stop();
    if (dragging) {
      // Letting go over the row that was pressed would click it (and tick a tick): a drag is not a click.
      const swallow = (click: MouseEvent) => {
        click.stopPropagation();
        click.preventDefault();
      };
      document.addEventListener('click', swallow, true);
      window.setTimeout(() => document.removeEventListener('click', swallow, true), 0);
    }
    if (where !== null) options.onDrop(where, how);
  }
  function cancel() {
    stop();
  }
  function key(ev: KeyboardEvent) {
    if (ev.key === 'Escape') {
      ev.stopPropagation();
      stop();
    }
  }
  document.addEventListener('pointermove', move);
  document.addEventListener('pointerup', up);
  document.addEventListener('pointercancel', cancel);
  document.addEventListener('keydown', key, true);
}
