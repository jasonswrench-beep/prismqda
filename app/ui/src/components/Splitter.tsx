import { useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { t } from '../i18n';

interface Props {
  /** 'vertical' is a vertical bar between a left and a right area; 'horizontal' sits between top and bottom. */
  orientation: 'vertical' | 'horizontal';
  label: string;
  /** Size in pixels of the area this bar resizes. */
  value: number;
  min: number;
  max: number;
  /** True when the resized area is to the right of (or below) the bar, so dragging toward the start makes it bigger. */
  reverse?: boolean;
  controls: string;
  onChange: (value: number) => void;
  /** Called when a drag or key press is finished, so the size can be remembered. */
  onCommit: (value: number) => void;
}

const STEP = 16;
const BIG_STEP = 64;

/** A dividing line between two areas. Drag it with the mouse, or focus it and use the arrow keys. */
export function Splitter(props: Props) {
  const { orientation, value, min, max, reverse } = props;
  const drag = useRef<{ from: number; start: number } | null>(null);
  const clamp = (n: number) => Math.min(Math.max(Math.round(n), props.min), Math.max(props.max, props.min));
  const percent = max > min ? Math.round(((value - min) / (max - min)) * 100) : 0;
  const horizontal = orientation === 'horizontal';

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { from: horizontal ? e.clientY : e.clientX, start: value };
    e.preventDefault();
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    const delta = (horizontal ? e.clientY : e.clientX) - d.from;
    props.onChange(clamp(d.start + (reverse ? -delta : delta)));
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    const delta = (horizontal ? e.clientY : e.clientX) - d.from;
    props.onCommit(clamp(d.start + (reverse ? -delta : delta)));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? BIG_STEP : STEP;
    const grow = horizontal ? (reverse ? 'ArrowUp' : 'ArrowDown') : reverse ? 'ArrowLeft' : 'ArrowRight';
    const shrink = horizontal ? (reverse ? 'ArrowDown' : 'ArrowUp') : reverse ? 'ArrowRight' : 'ArrowLeft';
    let next: number | null = null;
    if (e.key === grow) next = value + step;
    else if (e.key === shrink) next = value - step;
    else if (e.key === 'Home') next = min;
    else if (e.key === 'End') next = max;
    if (next === null) return;
    e.preventDefault();
    const v = clamp(next);
    props.onChange(v);
    props.onCommit(v);
  };

  return (
    <div
      role="separator"
      tabIndex={0}
      className={`splitter splitter-${orientation}`}
      aria-orientation={orientation === 'vertical' ? 'vertical' : 'horizontal'}
      aria-label={props.label}
      aria-controls={props.controls}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      aria-valuetext={horizontal ? t('{value} pixels tall', { value }) : t('{value} pixels wide', { value })}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
    />
  );
}
