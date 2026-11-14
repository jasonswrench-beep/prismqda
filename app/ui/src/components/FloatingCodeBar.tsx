import { useMemo } from 'react';
import { Button } from 'react-aria-components';
import { barChoices } from '../lib/floatbar';
import { applyFavorite, applyCode, inVivo } from '../state/actions';
import { focusApplyBox, useApp } from '../state/app';
import { rich, t } from '../i18n';

/**
 * The small row of buttons that floats above words selected with the mouse: the code used last, the favorites,
 * In vivo, and a way to choose any other code. It does what the keys and the toolbar do, no more.
 *
 * It is for the mouse. The buttons are left out of the Tab order and never take the focus, so the words stay
 * selected and the keys (1 to 9, R, I, Enter) go on working in the text. The keyboard has all of it already.
 */
export function FloatingCodeBar() {
  const codes = useApp((s) => s.codes);
  const favorites = useApp((s) => s.favorites);
  const lastCodeId = useApp((s) => s.lastCodeId);
  const choices = useMemo(() => barChoices(codes, favorites, lastCodeId), [codes, favorites, lastCodeId]);

  const swatch = (color: string) => <span className="code-dot" style={{ ['--swatch-color' as string]: color }} aria-hidden="true" />;

  return (
    <div className="float-bar-row" role="group" aria-label={t('Code the selected words')}>
      {choices.last ? (
        <Button
          className="float-button"
          excludeFromTabOrder
          preventFocusOnPress
          aria-label={t('Code the selection as {name}, the code used last', { name: choices.last.name })}
          onPress={() => void applyCode(choices.last!.id)}
        >
          {swatch(choices.last.color)}
          <span className="float-name" aria-hidden="true">
            {choices.last.name}
          </span>
          <kbd aria-hidden="true">R</kbd>
        </Button>
      ) : null}
      {choices.favorites.map((f) => (
        <Button
          key={f.slot}
          className="float-button"
          excludeFromTabOrder
          preventFocusOnPress
          aria-label={t('Code the selection as {name}, favorite {slot}', { name: f.code.name, slot: f.slot })}
          onPress={() => void applyFavorite(f.slot)}
        >
          {swatch(f.code.color)}
          <span className="float-name" aria-hidden="true">
            {f.code.name}
          </span>
          <kbd aria-hidden="true">{f.slot}</kbd>
        </Button>
      ))}
      {choices.others.map((c) => (
        <Button
          key={c.id}
          className="float-button"
          excludeFromTabOrder
          preventFocusOnPress
          aria-label={t('Code the selection as {name}', { name: c.name })}
          onPress={() => void applyCode(c.id)}
        >
          {swatch(c.color)}
          <span className="float-name" aria-hidden="true">
            {c.name}
          </span>
        </Button>
      ))}
      {choices.last || choices.favorites.length > 0 || choices.others.length > 0 ? <span className="float-sep" aria-hidden="true" /> : null}
      <Button className="float-button quiet" excludeFromTabOrder preventFocusOnPress aria-label={t('In vivo: make a code from the selected words')} onPress={() => void inVivo()}>{rich('In vivo <kbd>I</kbd>', { kbd: (inside) => <kbd aria-hidden="true">{inside}</kbd> })}
      </Button>
      <Button className="float-button quiet" excludeFromTabOrder preventFocusOnPress aria-label={t('Other code: choose any code to apply')} onPress={() => focusApplyBox()}>{rich('Other code <kbd>Enter</kbd>', { kbd: (inside) => <kbd aria-hidden="true">{inside}</kbd> })}
      </Button>
    </div>
  );
}
