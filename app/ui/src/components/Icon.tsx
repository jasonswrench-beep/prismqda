import { ICON_DEFS, type IconName } from '../assets/icons.generated';

export type { IconName };

/**
 * One icon from the PrismQDA set (assets/icons), drawn as ordinary SVG elements that take their color from the text
 * around them. It is decorative: the button or label it sits in carries the accessible name, so an icon-only button
 * still needs its own aria-label.
 *
 * Use the plain name for the outline icon and the "-active" name (workspace tabs only) for the filled, selected one.
 */
export function Icon(props: { name: IconName; size?: number; className?: string }) {
  const def = ICON_DEFS[props.name];
  const size = props.size ?? 18;
  const outline = def.filled
    ? ({ fill: 'currentColor', stroke: 'none' } as const)
    : ({ fill: 'none', stroke: 'currentColor', strokeWidth: 1.75, strokeLinecap: 'round', strokeLinejoin: 'round' } as const);
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox={def.viewBox}
      width={size}
      height={size}
      className={`icon${props.className ? ` ${props.className}` : ''}`}
      aria-hidden="true"
      focusable="false"
      {...outline}
    >
      {def.paths.map((p, i) => (
        <path key={i} d={p.d} fillRule={'fillRule' in p ? p.fillRule : undefined} fill={'fill' in p ? p.fill : undefined} stroke={'stroke' in p ? p.stroke : undefined} strokeDasharray={'dash' in p ? p.dash : undefined} />
      ))}
    </svg>
  );
}
