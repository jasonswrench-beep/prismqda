import type { CSSProperties } from 'react';
import { describeShape } from '../lib/regions';
import { useApp } from '../state/app';
import type { RegionShape } from '../types';
import { Icon } from './Icon';
import { RegionThumb } from './RegionThumb';

/**
 * The words of a coded passage as a quotation, or for a coded area of a picture a small picture of the area with its words
 * ("Rectangle, 10% to 40% across..."), or for a coded stretch of a recording its times ("Recording, 0:12 to 0:45"), so a list of
 * passages reads the same whatever kind of data each came from.
 */
export function SegmentQuote(props: { text: string; sourceId: number; region?: RegionShape | null; page?: number | null; max?: number; color?: string }) {
  const image = useApp((s) => s.sources.find((x) => x.id === props.sourceId)?.image ?? null);
  const recordingKind = useApp((s) => s.sources.find((x) => x.id === props.sourceId)?.recording?.kind ?? 'audio');
  const onPdf = props.page !== undefined && props.page !== null;
  const style = props.color ? ({ ['--seg-color' as string]: props.color } as CSSProperties) : undefined;
  if (props.region?.shape === 'span') {
    return (
      <blockquote className="bk-quote img-quote clip-quote" style={style} data-testid="segment-quote-clip">
        <span className="clip-glyph" aria-hidden="true"><Icon name={recordingKind === 'video' ? 'src-video' : 'src-audio'} size={28} /></span>
        <span>{describeShape(props.region)}</span>
      </blockquote>
    );
  }
  if (props.region && (image || onPdf)) {
    return (
      <blockquote className="bk-quote img-quote" style={style} data-testid="segment-quote-area">
        <RegionThumb sourceId={props.sourceId} image={image} page={props.page} shape={props.region} size={72} />
        <span>{props.text}</span>
      </blockquote>
    );
  }
  const words = props.text.trim();
  return (
    <blockquote className="bk-quote" style={style}>
      {`“${props.max ? words.slice(0, props.max) : words}”`}
    </blockquote>
  );
}
