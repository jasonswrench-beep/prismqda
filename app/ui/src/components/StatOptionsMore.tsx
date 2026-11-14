import { useStats, statsStore } from '../state/statistics';
import type { StatProcedure, StatRunsCut, StatVariable } from '../types';
import { valueLabel } from '../lib/statistics';
import { t } from '../i18n';
import { CURVE_KINDS, curveHint, curveName } from '../lib/statistics2';
import { Tick } from './Tick';

/**
 * The fields of the tests added after the first set: the curve of the Kolmogorov-Smirnov test, where the runs test cuts a number,
 * the success and the share of the binomial test, the expected shares of the one-sample chi-square, and the control group of the
 * Moses test. What shows depends on the test chosen.
 */
export function MoreOptions({ method, variable, groupChoices }: { method: StatProcedure; variable?: StatVariable; groupChoices: { value: string; label: string }[] }) {
  const s = useStats((x) => x);
  const set = (patch: Partial<typeof s>) => statsStore.set(patch);
  const categories = variable?.categories ?? [];
  const label = (c: { value: string; label: string }) => (variable ? valueLabel(variable.kind, c.value, c.label) : c.label);
  const shareTotal = categories.reduce((a, c) => a + (Number(s.shares[c.value]) || 0), 0);
  switch (method) {
    case 'ks_normal':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-ks-options">
          <legend>{t('The normal curve')}</legend>
          <div className="field">
            <label htmlFor="stats-ks-mean">{t('Mean of the curve')}</label>
            <input id="stats-ks-mean" type="number" step="any" value={s.mu} onChange={(e) => set({ mu: e.target.value })} data-testid="stats-ks-mean" />
          </div>
          <div className="field">
            <label htmlFor="stats-ks-sd">{t('Standard deviation of the curve')}</label>
            <input id="stats-ks-sd" type="number" step="any" min="0" value={s.sd} onChange={(e) => set({ sd: e.target.value })} aria-describedby="stats-ks-help" data-testid="stats-ks-sd" />
            <p id="stats-ks-help" className="help">{t("Leave the standard deviation empty to use the sample's own mean and standard deviation. The p-value is then too large, and the Shapiro-Wilk test is the better test of normality.")}</p>
          </div>
        </fieldset>
      );
    case 'runs':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-runs-options">
          <legend>{t('Cutting a number in two')}</legend>
          <div className="field">
            <label htmlFor="stats-cut">{t('Cut at')}</label>
            <select id="stats-cut" value={s.cut} onChange={(e) => set({ cut: e.target.value as StatRunsCut })} data-testid="stats-cut">
              <option value="median">{t('The median')}</option>
              <option value="mean">{t('The mean')}</option>
              <option value="value">{t('A value I give')}</option>
            </select>
          </div>
          {s.cut === 'value' ? (
            <div className="field">
              <label htmlFor="stats-cut-value">{t('The value')}</label>
              <input id="stats-cut-value" type="number" step="any" value={s.mu} onChange={(e) => set({ mu: e.target.value })} data-testid="stats-cut-value" />
            </div>
          ) : null}
          <p className="help">{t('A variable with two values needs no cut. The runs are counted in the order the cases have in the project.')}</p>
        </fieldset>
      );
    case 'curve_fit':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-curve-options">
          <legend>{t('Curves to fit')}</legend>
          {CURVE_KINDS.map((k) => (
            <Tick
              key={k}
              isSelected={s.curves.includes(k)}
              onChange={(on) => {
                const next = on ? CURVE_KINDS.filter((c) => c === k || s.curves.includes(c)) : s.curves.filter((c) => c !== k);
                if (next.length > 0) set({ curves: next });
              }}
              testId={`stats-curve-${k}`}
            >
              <strong>{curveName(k)}</strong> <span className="muted">{curveHint(k)}</span>
            </Tick>
          ))}
          <p className="help">{t('At least one curve stays ticked. A curve that cannot be fitted to these numbers (a logarithm of a number that is not above zero) is named in the results, beside the ones that were.')}</p>
        </fieldset>
      );
    case 'logistic_regression': {
      const chosen = categories.some((c) => c.value === s.success) ? s.success : (categories.find((c) => c.value === '1') ?? categories[categories.length - 1])?.value ?? '';
      return (
        <fieldset className="stats-fieldset" data-testid="stats-logistic-options">
          <legend>{t('The outcome to model')}</legend>
          <div className="field">
            <label htmlFor="stats-event">{t('Model the odds of')}</label>
            <select id="stats-event" value={chosen} onChange={(e) => set({ success: e.target.value })} aria-describedby="stats-event-help" data-testid="stats-event">
              {categories.map((c) => (
                <option key={c.value} value={c.value}>
                  {label(c)}
                </option>
              ))}
            </select>
            <p id="stats-event-help" className="help">{t('The odds ratios say how the odds of this value, against the other, change. Choosing the other value turns every odds ratio over (to its inverse).')}</p>
          </div>
        </fieldset>
      );
    }
    case 'binomial':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-binomial-options">
          <legend>{t('The share to test')}</legend>
          <div className="field">
            <label htmlFor="stats-success">{t('Count the cases that are')}</label>
            <select id="stats-success" value={s.success !== '' ? s.success : (categories[0]?.value ?? '')} onChange={(e) => set({ success: e.target.value })} data-testid="stats-success">
              {categories.map((c) => (
                <option key={c.value} value={c.value}>
                  {label(c)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="stats-proportion">{t('Compare their share with')}</label>
            <input id="stats-proportion" type="number" step="any" min="0" max="1" value={s.proportion} onChange={(e) => set({ proportion: e.target.value })} aria-describedby="stats-proportion-help" data-testid="stats-proportion" />
            <p id="stats-proportion-help" className="help">{t('A share between 0 and 1: 0.5 is half of the cases.')}</p>
          </div>
        </fieldset>
      );
    case 'goodness_of_fit':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-gof-options">
          <legend>{t('The shares to compare with')}</legend>
          <label className="stats-radio">
            <input type="radio" name="stats-shares" checked={s.equalShares} onChange={() => set({ equalShares: true })} data-testid="stats-shares-equal" />
            <span>{t('Equal shares: every category expected to have the same number of cases')}</span>
          </label>
          <label className="stats-radio">
            <input type="radio" name="stats-shares" checked={!s.equalShares} onChange={() => set({ equalShares: false })} data-testid="stats-shares-typed" />
            <span>{t('Shares I give')}</span>
          </label>
          {!s.equalShares ? (
            <>
              {categories.map((c) => (
                <div className="field" key={c.value}>
                  <label htmlFor={`stats-share-${c.value}`}>{t('Share of {category}', { category: label(c) })}</label>
                  <input id={`stats-share-${c.value}`} type="number" step="any" min="0" value={s.shares[c.value] ?? ''} onChange={(e) => set({ shares: { ...s.shares, [c.value]: e.target.value } })} data-testid={`stats-share-${c.value}`} />
                </div>
              ))}
              <p className="help">{t('Write shares as percentages or as any numbers in proportion, such as 50, 30, and 20. They are scaled to add up to 1. The total now is {total}.', { total: String(Math.round(shareTotal * 1000) / 1000) })}</p>
            </>
          ) : null}
        </fieldset>
      );
    case 'moses':
      return (
        <fieldset className="stats-fieldset" data-testid="stats-moses-options">
          <legend>{t('The control group')}</legend>
          <div className="field">
            <label htmlFor="stats-control">{t('The control group is')}</label>
            <select id="stats-control" value={s.control !== '' ? s.control : (groupChoices[0]?.value ?? '')} onChange={(e) => set({ control: e.target.value })} data-testid="stats-control">
              {groupChoices.map((g) => (
                <option key={g.value} value={g.value}>
                  {g.label}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="stats-trim">{t('Control cases to leave out at each end')}</label>
            <input id="stats-trim" type="number" step="1" min="0" value={s.trim} onChange={(e) => set({ trim: e.target.value })} aria-describedby="stats-trim-help" data-testid="stats-trim" />
            <p id="stats-trim-help" className="help">{t('Leave it empty for 5 percent of the control cases, and at least 1. Trimming keeps one stray case from deciding the result.')}</p>
          </div>
        </fieldset>
      );
    default:
      return null;
  }
}
