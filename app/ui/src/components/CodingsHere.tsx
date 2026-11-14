import { Button } from 'react-aria-components';
import { quoted } from '../lib/eventWording';
import { sliceScalar, type OffsetIndex } from '../offsets';
import { useApp } from '../state/app';
import type { CodeVisual } from '../surface/TextSurface';
import type { CodingRow } from '../types';
import { rich, t, tn } from '../i18n';

interface Props {
  scopeLabel: string;
  scopeKind: 'unit' | 'paragraph' | 'none';
  codings: readonly CodingRow[];
  text: string;
  index: OffsetIndex;
  visuals: ReadonlyMap<number, CodeVisual>;
  /** Ids of codings that contain the caret. */
  caretCodes: readonly number[];
  onOpen: (coding: CodingRow) => void;
}

const MAX_SHOWN = 40;

/**
 * The text equivalent of the margin stripes: a plain list of the codings in the current paragraph
 * or unit, with the code's name and the words it covers.
 */
export function CodingsHere(props: Props) {
  const selectedCodingId = useApp((s) => s.selectedCodingId);
  const anchors = useApp((s) => s.anchors);
  const shown = props.codings.slice(0, MAX_SHOWN);
  const atCaret = new Set(props.caretCodes);
  return (
    <details className="codings-here bk-here" aria-labelledby="h-here" data-testid="codings-here">
      <summary>
        {props.scopeKind !== 'none'
          ? rich('<span>Codings here</span><span2>{scopeLabel}, {codings}</span2>', { scopeLabel: props.scopeLabel, codings: tn(shown.length, { one: '{count} coding', other: '{count} codings' }), span: (inside) => <span id="h-here">{inside}</span>, span2: (inside) => <span className="muted scope-label">{inside}</span> })
          : rich('<span>Codings here</span><span2>{scopeLabel}</span2>', { scopeLabel: props.scopeLabel, span: (inside) => <span id="h-here">{inside}</span>, span2: (inside) => <span className="muted scope-label">{inside}</span> })}
      </summary>
      {props.scopeKind === 'none' ? (
        <p className="muted">{t('Click or move into the text to see its codings.')}</p>
      ) : shown.length === 0 ? (
        <p className="muted" data-testid="codings-here-empty">{t('No codings in this {scopeKind}.', { scopeKind: props.scopeKind })}</p>
      ) : (
        <ul className="here-list" data-testid="codings-here-list">
          {shown.map((c) => {
            const v = props.visuals.get(c.code_id);
            const words = sliceScalar(props.text, props.index, c.start, Math.min(c.end, c.start + 160));
            return (
              <li key={c.id}>
                <Button className={`here-item${c.id === selectedCodingId ? ' chosen' : ''}`} onPress={() => props.onOpen(c)} data-coding-id={c.id}>
                  <span className="here-code">{v?.name ?? t('Unknown code')}</span>
                  <span className="here-quote">{quoted(words, 90)}</span>
                  <span className="here-flags">
                    {atCaret.has(c.id) ? <span className="tag">{t('at the caret')}</span> : null}
                    {c.rationale.trim() ? <span className="tag">{t('has a note')}</span> : null}
                    {anchors.has(c.id) ? <span className="tag">{t('example')}</span> : null}
                  </span>
                </Button>
              </li>
            );
          })}
          {props.codings.length > MAX_SHOWN ? <li className="muted">{t('and {count} more', { count: props.codings.length - MAX_SHOWN })}</li> : null}
        </ul>
      )}
    </details>
  );
}
