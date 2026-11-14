import fullMark from '../../../../assets/brand/brand-mark.svg';
import smallMark from '../../../../assets/brand/brand-mark-small.svg';

/**
 * The PrismQDA prism. Decorative: the product name is always written out beside it, so it has an empty alt text.
 * The small version is for 16 to 32 pixels; the full one is for anything larger.
 */
export function BrandMark(props: { size: number; className?: string }) {
  const src = props.size <= 32 ? smallMark : fullMark;
  return <img src={src} alt="" width={props.size} style={{ height: 'auto' }} className={`brand-mark${props.className ? ` ${props.className}` : ''}`} draggable={false} />;
}
