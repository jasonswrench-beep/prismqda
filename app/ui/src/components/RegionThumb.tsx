import { useEffect, type ReactElement } from 'react';
import { imageUrl, pageWidthFor, pdfPageUrl } from '../bridge';
import { pagePixels } from '../lib/pdfPages';
import { polygonPoints, thumbPlacement } from '../lib/regions';
import { loadPdfInfo } from '../state/actions';
import { useApp } from '../state/app';
import type { AreaShape, SourceImage } from '../types';

/**
 * A small picture of one coded area: the part of the picture (or of the page of a PDF, when `page` is given) it covers, with its
 * outline. It is decoration beside words that already say what the area is, so it is hidden from a screen reader.
 */
export function RegionThumb(props: { sourceId: number; image?: Pick<SourceImage, 'width' | 'height' | 'size_bytes'> | null; page?: number | null; shape: AreaShape; size?: number }) {
  const { sourceId, shape } = props;
  const size = props.size ?? 48;
  const onPdf = props.page !== undefined && props.page !== null;
  const pdf = useApp((s) => (onPdf ? (s.pdfInfo[sourceId] ?? null) : null));
  useEffect(() => {
    if (onPdf && !pdf) void loadPdfInfo(sourceId);
  }, [onPdf, pdf, sourceId]);
  // What is drawn: the picture itself, or the page of the PDF at a size that fills the thumbnail with the area.
  const pageSize = onPdf && pdf ? pdf.pages[(props.page as number) - 1] : null;
  const picture = onPdf ? (pageSize ? { ...pagePixels(pageSize), size_bytes: pdf!.size_bytes } : null) : (props.image ?? null);
  if (!picture) return <span className="img-thumb" style={{ width: size, height: size }} aria-hidden="true" data-testid="region-thumb" />;
  const p = thumbPlacement(shape, picture.width, picture.height, size, size);
  const src = onPdf ? pdfPageUrl(sourceId, props.page as number, pageWidthFor(p.width), picture.size_bytes) : imageUrl(sourceId, picture.size_bytes);
  // The outline is drawn on the same placement, in the thumbnail's own pixels.
  let outline: ReactElement;
  if (shape.shape === 'rectangle') outline = <rect x={p.left + shape.x * p.width} y={p.top + shape.y * p.height} width={shape.w * p.width} height={shape.h * p.height} />;
  else if (shape.shape === 'ellipse') outline = <ellipse cx={p.left + (shape.x + shape.w / 2) * p.width} cy={p.top + (shape.y + shape.h / 2) * p.height} rx={(shape.w / 2) * p.width} ry={(shape.h / 2) * p.height} />;
  else outline = <polygon points={polygonPoints(shape.points.map(([x, y]) => [x, y] as [number, number]), p.width, p.height).split(' ').map((pair) => {
    const [x, y] = pair.split(',').map(Number);
    return `${x + p.left},${y + p.top}`;
  }).join(' ')} />;
  return (
    <span className="img-thumb" style={{ width: size, height: size }} aria-hidden="true" data-testid="region-thumb">
      <img src={src} alt="" draggable={false} loading="lazy" style={{ width: p.width, height: p.height, left: p.left, top: p.top }} />
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} focusable="false">
        <g className="halo">{outline}</g>
        <g className="line">{outline}</g>
      </svg>
    </span>
  );
}
