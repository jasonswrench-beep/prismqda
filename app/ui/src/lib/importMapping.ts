// The spreadsheet import wizard: what each column is for, and the mapping the engine wants.

import type { Level, SurveyMapping, TablePreview } from '../types';
import { t } from '../i18n';

export type ColumnRole = 'unused' | 'id' | 'unit_id' | 'text' | 'attribute' | 'unit_attribute';

export interface ColumnChoice {
  role: ColumnRole;
  /** Level of measurement; only used when the role is an attribute. */
  level: Level;
}

export type Choices = Record<string, ColumnChoice>;

export const ROLE_LABELS: Record<ColumnRole, string> = {
  get unused() { return t('Leave out'); },
  get id() { return t('Person ID'); },
  get unit_id() { return t('ID of each answer'); },
  get text() { return t('Open-ended text'); },
  get attribute() { return t('About the person'); },
  get unit_attribute() { return t('About each answer'); },
};

export const LEVEL_LABELS: Record<Level, string> = {
  get nominal() { return t('Categories (nominal)'); },
  get ordinal() { return t('Ordered categories (ordinal)'); },
  get interval() { return t('Numbers with equal steps (interval)'); },
  get ratio() { return t('Numbers with a true zero (ratio)'); },
  get date() { return t('Date'); },
  get text() { return t('Free text'); },
};

export const LEVELS: Level[] = ['nominal', 'ordinal', 'interval', 'ratio', 'date', 'text'];

/** Starts from the engine's own suggestion, so the first preview is what the engine would do alone. */
export function choicesFromSuggestion(table: TablePreview, suggested: SurveyMapping): Choices {
  const out: Choices = {};
  for (const col of table.columns) out[col.name] = { role: 'unused', level: col.guessed_level };
  if (suggested.id_column && out[suggested.id_column]) out[suggested.id_column].role = 'id';
  if (suggested.unit_id_column && out[suggested.unit_id_column]) out[suggested.unit_id_column].role = 'unit_id';
  for (const name of suggested.text_columns ?? []) if (out[name]) out[name].role = 'text';
  for (const a of suggested.attribute_columns ?? []) {
    if (out[a.name]) {
      out[a.name].role = 'attribute';
      if (a.level) out[a.name].level = a.level;
    }
  }
  for (const a of suggested.unit_attribute_columns ?? []) {
    if (out[a.name]) {
      out[a.name].role = 'unit_attribute';
      if (a.level) out[a.name].level = a.level;
    }
  }
  return out;
}

/** Sets one column's role. Only one column can be the person ID and one the answer ID, so a new one replaces the old. */
export function setRole(choices: Choices, column: string, role: ColumnRole): Choices {
  const next: Choices = {};
  for (const [name, c] of Object.entries(choices)) {
    const replaced = (role === 'id' || role === 'unit_id') && c.role === role && name !== column;
    next[name] = replaced ? { ...c, role: 'unused' } : { ...c };
  }
  next[column] = { ...(next[column] ?? { role, level: 'nominal' }), role };
  return next;
}

export function setLevel(choices: Choices, column: string, level: Level): Choices {
  return { ...choices, [column]: { ...choices[column], level } };
}

/** Plain words about what is wrong with the choices, or null if the engine can be asked. */
export function validateChoices(choices: Choices): string | null {
  const roles = Object.values(choices).map((c) => c.role);
  if (!roles.includes('text')) return t('Choose at least one column that holds open-ended text, the words people wrote.');
  return null;
}

export function mappingFromChoices(table: TablePreview, choices: Choices): SurveyMapping {
  const mapping: SurveyMapping = { id_column: null, text_columns: [], attribute_columns: [], unit_id_column: null, unit_attribute_columns: [] };
  for (const col of table.columns) {
    const c = choices[col.name];
    if (!c) continue;
    switch (c.role) {
      case 'id':
        mapping.id_column = col.name;
        break;
      case 'unit_id':
        mapping.unit_id_column = col.name;
        break;
      case 'text':
        mapping.text_columns.push(col.name);
        break;
      case 'attribute':
        mapping.attribute_columns!.push({ name: col.name, level: c.level });
        break;
      case 'unit_attribute':
        mapping.unit_attribute_columns!.push({ name: col.name, level: c.level });
        break;
      default:
        break;
    }
  }
  return mapping;
}
