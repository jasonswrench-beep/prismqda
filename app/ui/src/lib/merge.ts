import type { MergeReport } from '../types';
import { list, num, t, tn } from '../i18n';

/** Why codings were left out of a merge, as one sentence for the number left out. */
export function skippedSentence(reason: string, count: number): string {
  switch (reason) {
    case 'code_retired':
      return tn(count, { one: '{count} coding was left out, because its code was merged or retired in the copy.', other: '{count} codings were left out, because their code was merged or retired in the copy.' });
    case 'code_unmatched':
      return tn(count, { one: '{count} coding was left out, because its code could not be matched.', other: '{count} codings were left out, because their code could not be matched.' });
    case 'coder_unmatched':
      return tn(count, { one: '{count} coding was left out, because its coder could not be matched.', other: '{count} codings were left out, because their coder could not be matched.' });
    case 'no_unit':
      return tn(count, { one: '{count} coding was left out, because its words fit no unit of this project, which codes whole units.', other: '{count} codings were left out, because their words fit no unit of this project, which codes whole units.' });
    case 'exclusive_group':
      return tn(count, { one: '{count} coding was left out, because it clashes with a mutually exclusive group here.', other: '{count} codings were left out, because they clash with a mutually exclusive group here.' });
    default:
      return tn(count, { one: '{count} coding was left out, because this project would not take it.', other: '{count} codings were left out, because this project would not take them.' });
  }
}

/** What a merge did or would do, as sentences in the order a person reads them: documents, coders, codes, codings, areas, memos. */
export function mergeSentences(r: MergeReport): string[] {
  const out: string[] = [];
  out.push(r.documents_added.length === 0 ? t('No document is added.') : t('Documents added: {names}.', { names: list(r.documents_added) }));
  if (r.documents_matched > 0) out.push(tn(r.documents_matched, { one: '{count} document is in both projects with the same words.', other: '{count} documents are in both projects with the same words.' }));
  if (r.documents_in_conflict.length > 0) {
    out.push(t('Left out, because a document of the same name has other words here: {names}. Rename one of them to bring it in.', { names: list(r.documents_in_conflict) }));
  }
  out.push(r.coders_added.length === 0 ? t('No coder is added.') : t('Coders added: {names}.', { names: list(r.coders_added) }));
  out.push(r.codes_added.length === 0 ? t('No code is added.') : t('Codes added: {names}.', { names: list(r.codes_added) }));
  out.push(t('Codings added: {added}. Already here: {already}.', { added: num(r.codings_added), already: num(r.codings_already_here) }));
  for (const s of r.codings_skipped) out.push(skippedSentence(s.reason, s.count));
  if (r.areas_added + r.areas_already_here > 0) out.push(t('Areas of pictures added: {added}. Already here: {already}.', { added: num(r.areas_added), already: num(r.areas_already_here) }));
  if (r.links_added + r.links_already_here > 0) out.push(t('Links between passages added: {added}. Already here: {already}.', { added: num(r.links_added), already: num(r.links_already_here) }));
  if (r.memos_added + r.memos_already_here > 0) out.push(t('Memos added: {added}. Already here: {already}.', { added: num(r.memos_added), already: num(r.memos_already_here) }));
  if (r.cases_added.length > 0) out.push(t('Cases added: {names}.', { names: list(r.cases_added) }));
  if (r.case_members_added > 0) out.push(tn(r.case_members_added, { one: '{count} document was put in a case here, because it is in that case in the copy.', other: '{count} documents were put in a case here, because they are in that case in the copy.' }));
  if (r.variables_added.length > 0) out.push(t('Variables of cases added: {names}.', { names: list(r.variables_added) }));
  if (r.values_added > 0) out.push(tn(r.values_added, { one: '{count} value of a case was added.', other: '{count} values of cases were added.' }));
  if (r.variables_in_conflict.length > 0) {
    out.push(t('The values of these variables were left out, because a variable of the same name is at another level here, or cannot hold them: {names}.', { names: list(r.variables_in_conflict) }));
  }
  if (r.values_replaced > 0) out.push(tn(r.values_replaced, { one: '{count} value of a case was changed to the one in the copy, as you chose.', other: '{count} values of cases were changed to the ones in the copy, as you chose.' }));
  if (r.definitions_replaced.length > 0) out.push(t('Wording taken from the copy, as you chose: {names}.', { names: list(r.definitions_replaced) }));
  if (r.values_in_conflict.length > 0) {
    const shown = r.values_in_conflict.slice(0, 5).map((c) => t('{case}, {variable} (here {ours}, in the copy {theirs})', { case: c.case, variable: c.variable, ours: c.ours, theirs: c.theirs }));
    const more = r.values_in_conflict.length - shown.length;
    out.push(
      tn(r.values_in_conflict.length, { one: '{count} value of a case differs from the one in the copy, and the one here was kept: {which}.', other: '{count} values of cases differ from the ones in the copy, and the ones here were kept: {which}.' }, { which: list(shown) + (more > 0 ? t(', and {count} more', { count: num(more) }) : '') }),
    );
  }
  return out;
}

/** True when a merge would add nothing: the copy holds nothing this project does not already have. */
export function addsNothing(r: MergeReport): boolean {
  return r.documents_added.length + r.coders_added.length + r.codes_added.length + r.codings_added + r.areas_added + r.memos_added + r.links_added + r.cases_added.length + r.variables_added.length + r.values_added + r.case_members_added + r.values_replaced + r.definitions_replaced.length === 0;
}
