import type { AgreementCoder, AgreementCodeReport, AgreementInterval, AgreementReport } from '../types';
import { num, t, tn } from '../i18n';

/** The coefficients a report can give, in the order the Team page lists them. */
export type CoefficientKey = 'krippendorff_alpha' | 'cohen_kappa' | 'scott_pi' | 'fleiss_kappa' | 'brennan_prediger' | 'gwet_ac1' | 'percent_agreement';

export const COEFFICIENTS: { key: CoefficientKey; label: string }[] = [
  { key: 'krippendorff_alpha', get label() { return t("Krippendorff's alpha"); } },
  { key: 'cohen_kappa', get label() { return t("Cohen's kappa"); } },
  { key: 'scott_pi', get label() { return t("Scott's pi"); } },
  { key: 'fleiss_kappa', get label() { return t("Fleiss' kappa"); } },
  { key: 'brennan_prediger', get label() { return t("Brennan and Prediger's kappa"); } },
  { key: 'gwet_ac1', get label() { return t("Gwet's AC1"); } },
  { key: 'percent_agreement', get label() { return t('Percent agreement'); } },
];

/** A coefficient to three places, or the words that say it cannot be worked out for these ratings. */
export function fixed(v: number | null | undefined): string {
  return v === null || v === undefined || !Number.isFinite(v) ? t('not defined') : num(v, { minimumFractionDigits: 3, maximumFractionDigits: 3 });
}

/** The interval round a coefficient, as words; empty when the run made none. */
export function intervalText(iv: AgreementInterval | undefined): string {
  return iv ? t('{low} to {high}', { low: fixed(iv.low), high: fixed(iv.high) }) : '';
}

/** What the interval is, said once under the table: its level and how many resamples made it. */
export function intervalNote(report: AgreementReport): string {
  const first = Object.values(report.intervals)[0];
  if (!first) return '';
  return t('Intervals cover {level} percent of {count} resamples of the units.', { level: num(Math.round(first.level * 100)), count: num(first.replications) });
}

/** A coder's name, with the words that say a model is not a person. */
export function coderLabel(c: Pick<AgreementCoder, 'name' | 'role'>): string {
  return c.role === 'model' ? t('{name} (AI model)', { name: c.name }) : c.name;
}

/** The word a report uses for what it measured. */
export function termLabel(term: string): string {
  return term === 'concordance' ? t('concordance') : t('intercoder reliability');
}

/** True when the coders chosen can be compared: two or more, each once. */
export function canMeasure(coderIds: number[]): boolean {
  return new Set(coderIds).size >= 2;
}

/** One sentence that says how the coders agreed, with the target, for the top of the result and for a screen reader. */
export function headline(r: AgreementReport): string {
  const units = tn(r.units_in_sample, { one: '{count} unit', other: '{count} units' });
  const target = fixed(r.target);
  if (r.overall) {
    const alpha = r.overall.krippendorff_alpha;
    if (alpha === null || alpha === undefined) {
      return t("{units} compared. Krippendorff's alpha is not defined for these ratings; percent agreement is {percent}.", { units, percent: fixed(r.overall.percent_agreement) });
    }
    return alpha >= r.target
      ? t("{units} compared. Krippendorff's alpha is {alpha}, which meets the target of {target}.", { units, alpha: fixed(alpha), target })
      : t("{units} compared. Krippendorff's alpha is {alpha}, which is below the target of {target}.", { units, alpha: fixed(alpha), target });
  }
  const below = r.by_code.filter((c) => c.below_target).length;
  if (r.by_code.length === 0) return t('{units} compared, but no code was applied.', { units });
  return below === 0
    ? t('{units} compared, code by code. Every code meets the target of {target}.', { units, target })
    : t('{units} compared, code by code. {below} of {codes} are below the target of {target}.', {
        units,
        below: num(below),
        codes: tn(r.by_code.length, { one: '{count} code', other: '{count} codes' }),
        target,
      });
}

/** The measure a code is judged by: alpha, or plain agreement when alpha cannot be worked out. */
export function codeMeasure(c: AgreementCodeReport): number | null {
  return c.detail.agreement.krippendorff_alpha ?? c.detail.agreement.percent_agreement ?? null;
}

/** The share of the units the coders disagreed on, as a whole-number percent, for the line under the confusion table. */
export function disagreementShare(r: AgreementReport): string {
  const compared = r.overall?.units_compared ?? r.units_in_sample;
  if (!compared) return '';
  return t('{count} of {total} units, {percent} percent.', { count: num(r.disagreements), total: num(compared), percent: num(Math.round((r.disagreements / compared) * 100)) });
}
