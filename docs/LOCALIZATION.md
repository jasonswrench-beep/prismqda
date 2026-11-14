# Languages

PrismQDA is written in English. Every word the window says goes through a **language catalog**, so another language can be added by translating a file, with no change to the program. A person can bring in a translation made elsewhere and label it (see Bringing in a translation); PrismQDA has no editor for translations. No translation has been made yet. Nothing is translated by a machine behind the person's back; a translation holds only what someone wrote, and says how it was made.

This page says how the catalog works, how a language is added, and what is not yet covered.

## How a message is found

A message is its own English text. The English text is the key, and it is also what shows when a language has no translation of it, so a missing translation never leaves a blank or a code on the screen, and a language can be added a message at a time.

| In the code | Means |
| --- | --- |
| `t('Save')` | a message |
| `t('Delete {name}?', { name })` | a message with a blank to fill. A blank is a letters-and-digits name in braces. |
| `tc('menu', 'Open')` | the same English words that mean different things get a note (a context), so each can be translated on its own |
| `tn(count, { one: '{count} code', other: '{count} codes' })` | a message with a count. `{count}` is filled in with the count written as the language writes numbers. The forms a language needs are the CLDR plural categories: `zero`, `one`, `two`, `few`, `many`, `other`. English needs two. Arabic needs all six. |
| `rich('Press <kbd>Enter</kbd> to code {name}', { kbd: (inside) => <kbd>{inside}</kbd>, name })` | a message whose words include marked stretches that the program styles. The translation decides where the marked words go. |
| `N_('Home')` | marks a message in a table so that it is collected, and returns the same text. Where the table is used, `t(value)` looks it up. |

`{{` and `}}` are a plain `{` and `}`. Inside a message, `<b>...</b>` is a mark, so a message that shows a `<` should be avoided.

A table that sits at the top of a file (a list of labels, a ribbon, the names of colors) must not call `t()` when the file loads, because the language is not chosen yet. Such a table is written with **getters**, which are read when they are used:

```ts
export const GROUP_LABELS = { get project() { return t('Project'); }, get documents() { return t('Documents'); } };
```

Numbers, lists, dates, and sorting follow the language too: `num(n)`, `list(items)`, `dateTime(date, options)`, `compareText(a, b)`. The check refuses `toLocaleString`, `localeCompare`, and the like in the interface, because they follow the computer's language instead of the window's.

### Rules for writing a message

1. **A whole sentence is one message.** Do not build a sentence from pieces that are joined in English order (`'the code is ' + not + names`): another language puts the words elsewhere. Write each whole sentence, with blanks.
2. **A count is `tn`, never `n === 1 ? 'code' : 'codes'`.**
3. **Do not compare words that are shown.** If a label is translated, `label === 'Today'` stops working. Compare an id.
4. **Name a blank for what goes in it** (`{name}`, `{file}`, `{count}`), because the translator reads the name.
5. **A word the window never changes** (the program's name, the name of a file kind such as PDF, a key such as Ctrl) is left outside the catalog and listed in `scripts/i18n/allow.json` with the reason.

## Sentences from the engine

The engine (the Rust part) says a few things in English that the window shows as they come: why a request was refused, a note under a table, a warning after an import. `npm run i18n:engine` collects them from the Rust source into `src/i18n/engine-messages.json` as templates with named blanks ("there is already a code named '{name}' at the new place"), and they go into the catalog like every other message. The window finds a sentence by its shape (`src/i18n/engine.ts`) and shows its translation with the blanks filled. A sentence the collector does not know is shown in English. The check fails when the Rust source and `engine-messages.json` disagree, so a new engine sentence cannot be forgotten.

A few engine sentences are built from fragments (`{count} entr{value} in the mailbox ...`); they are in the catalog with an odd blank, and a translator may need to translate them as the English shows. They are rare (import warnings).

## Bringing in a translation

PrismQDA does not translate and has no editor for translations. A translation is a file that someone made elsewhere, and PrismQDA reads it, checks it, and shows the window in it. A person who uses PrismQDA does this from **Settings > Language**:

1. **Save a file for a translator…** writes a spreadsheet (CSV) or a JSON file with the words of the window: the English in one column and an empty column for the translation. The first screens a person meets come first, so a translation can be useful before it is finished. [TRANSLATOR-GUIDE.md](TRANSLATOR-GUIDE.md) is the document to hand to the translator; it explains the columns, the `{blanks}`, the marks, the count forms, and has a glossary of the main words.
2. The translator fills the empty column in Excel or Google Sheets (saved as "CSV UTF-8") or edits the JSON.
3. **Add a translation…** reads the returned file. The file is checked **before** anything is kept, and the window says what it found: how many messages are translated, which entries were left out and why (a `{blank}` that was changed or lost, marks that differ, a count form the language does not use), how many are for messages this version does not have, and whether the file was saved in the old Windows encoding (accents may be wrong). Entries that fail are left out and the English shows for them, so a file with a few mistakes is usable.
4. The person chooses the **language** from the standard list, a **label** (what the list of languages shows, for example "Español (México), revisado por la clase de 2026"), **who translated it**, and **how it was made**: a person; a person who reviewed a machine translation; or a machine, not checked. The label and the words about how it was made are shown beside the language in Settings, so a team can see what they are using.
5. **Show the window in it now** switches at once. The choice is remembered, and a translation of a language that the computer uses is found by **Same as Windows**.

A translation is kept on this computer with the person's settings, not in a project, because the language of the window belongs to the person and not to the study. A translation can be **removed** (after asking), and **Save a copy** writes it out again as a spreadsheet or JSON file to give to someone or to correct and add again. A second translation of the same language can replace the first or be kept beside it. When a later version of PrismQDA adds messages, a translation shows English for them, and **Save a file for a translator** with "Only the messages not translated yet" gives the translator just the new rows. (A message is identified by its English text, so rewording an English message loses its translations; messages should be reworded only when they must be.)

### The standard list

The list holds about seventy languages and regional variants by language code (`pt-BR`, `zh-Hans`, `es-419`). Each is shown by its name in its own language and in English. The names, the direction of the text, and the plural forms a language needs all come from the browser, so no table of translated language names is kept. A translation file says which language it is for by one of these codes, or the person chooses it.

### File formats

- **Spreadsheet (CSV).** Columns **Key**, **Context**, **Form**, **English**, **Translation**, **Keep these as they are**; extra columns are ignored. It is written as UTF-8 with a byte-order mark, a first line `sep=,` that tells Excel the delimiter whatever the country's settings, and text that a spreadsheet would read as a formula (it starts with `=`, `+`, `-`, or `@`) is given a leading tab, which is taken off again when the file is read. A message with a count has one row for each form the language needs (`one`, `few`, `many`, `other`...). A file may use commas, semicolons, or tabs (found from its first line), and may be UTF-8, UTF-16 (Excel's "Unicode Text"), or Windows-1252 (Excel's plain "CSV"; the window warns).
- **JSON.** The same shape as the languages that come with the program: `locale`, optional `label`, `credit`, `madeBy`, `messages`, and `contexts`; a count message is an object of forms.

## Adding a language to the program itself

A language that should come with every copy of PrismQDA (instead of being added by the person) is a file in the source:

1. `npm run i18n:new -- fr "Français"` (add `--rtl` for a right-to-left language) writes `src/i18n/locales/fr.json` with every message and an empty translation, and adds the language to `src/i18n/manifest.json`.
2. Fill in the translations, keeping every `{blank}` and `<mark>`.
3. `npm run i18n:check` says what is wrong (a missing blank, a form the language does not use, a message that no longer exists) and how much is done.
4. When the program gains messages, `npm run i18n:new -- fr --update` adds them, empty, and removes nothing.

Everything the component library puts on the screen by itself (the label of a dismiss button, "0 options available") is supplied by the library in the language of the window.

## The test language

`?lang=en-XA` in the address shows the program in a test language: every message that goes through the catalog is bent into accented letters, framed in `⟦ ⟧`, and padded by a third (languages are often longer than English), with the blanks kept. A word still in plain English is a word that is not in the catalog. The browser check (`PRISMQDA_ONLY=language npm run browser-check` runs only this part) opens the program in the test language, visits every page, ribbon tab, and dialog, and lists every visible word and every label read aloud that is still plain English. The person's own words (the names of codes and documents, the text being coded) are the project's, not the window's, and are left out.

What the scan cannot see: a screen that no scenario opens, a word that appears only in an error, and the words inside exports (below).

## Not covered yet

- **Exports.** The Word, Excel, and CSV files made by the export code have English column headings and fixed phrases (for example "[Area of a picture]", "the picture is not shown in this file", and the description a screen reader speaks for a cropped area, "Area of the picture ..., coded ...: ..."); the codebook and report text are the person's.
- **The AI prompt packet** is text for a chatbot and stays English; whether it should follow the window's language is a decision to make with the people who use it.
- **The sample project** (Urban Mobility Study) is a made-up study in English; it is content, not the window.
- **Short labels built with a comma.** A sentence with an optional part is now written as whole sentences (with and without the part), or the optional part is a sentence of its own, so a language can order its words. What is left are short labels that add a phrase after a comma or in brackets ("Name, a person", ", 12:30 long", "(on the map)", "; the first 50 are shown"). They are in the catalog piece by piece; a translator may find a few awkward and should say which.
- **Date formats** follow the browser's rules for the language, which can differ from a translator's expectation.
- **Help.** There is no separate manual; the Help window lists the keyboard shortcuts, which are in the catalog.
- **Fonts.** The bundled font (Inter) covers Latin, Greek, and Cyrillic; a language in another script uses the font Windows supplies.

## Files

| File | What it is |
| --- | --- |
| `src/i18n/index.ts` | `t`, `tc`, `tn`, `N_`, `rich`, `num`, `list`, `dateTime`, `compareText`, and the language in use |
| `src/i18n/core.ts` | filling blanks, choosing a plural form, the test language, choosing a language |
| `src/i18n/locales.ts`, `manifest.json`, `locales/*.json` | the languages that come with the program, and the list of languages to choose from |
| `src/i18n/installed.ts`, `manage.ts` | the translations a person added on this computer; reading, adding, saving, and removing them |
| `src/i18n/pack.ts`, `csv.ts`, `languages.ts`, `tokens.ts` | reading and checking a translation file, writing the files a translator starts from, the standard list of languages |
| `src/components/LanguageBody.tsx`, `LanguageDialogs.tsx` | the Language part of Settings and its two windows |
| `src/i18n/engine.ts`, `engine-messages.json` | sentences from the engine |
| `src/i18n/messages.json` | every message of the program, for translators (made by `npm run i18n:extract`) |
| `scripts/i18n/` | the tools: `convert` (the one-time move of the interface into the catalog), `extract`, `audit`, `check`, `template`, `engine-extract` |
