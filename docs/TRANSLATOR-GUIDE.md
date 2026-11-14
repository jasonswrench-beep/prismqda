# Translating PrismQDA: a guide for translators

Thank you for translating. You do not need to know how PrismQDA is built. You need to read English well, write your language well, and have read the few rules below once.

## What PrismQDA is

PrismQDA is a free program for **qualitative research**: a researcher imports interviews, open-ended survey answers, or documents, reads them, and marks passages with **codes** (short names for ideas, such as "Trust" or "Cost"). Later the researcher asks questions of that coding: which codes occur together, in which documents, for which people. The people who use it are students, teachers, and researchers, many of whom are doing this for the first time. Some use a screen reader, so every button and field is read aloud.

## How to write

- **Plain and direct.** Short sentences. Speak to the person ("Choose a file"), not about them. No marketing words.
- **The way a careful colleague would say it**, not a word-for-word copy. If your language puts the verb last or the name first, write it that way.
- **Buttons and labels are short.** A button has little room; if a long translation is the only accurate one, use it, but prefer the short word people already know from other programs.
- **Labels are read aloud.** Write them as words. Do not use symbols in place of words.
- **Be consistent.** Use the same word every time for the same idea. The glossary below fixes the important ones; agree your own list of the ten or twenty most used words before you start.

## The file you are given

A spreadsheet with these columns (or a JSON file with the same content):

| Column | What it is | What you do |
| --- | --- | --- |
| **Key** | The English text that identifies the message | Never change it |
| **Context** | A note that tells two uses of the same English words apart (often empty) | Never change it |
| **Form** | Empty for most messages. For a message with a number it says which form this row is (`one`, `other`, and so on, see below) | Never change it |
| **English** | What the message says now | Read it |
| **Translation** | Empty | **Write your translation here** |
| **Keep these as they are** | The `{names}` and `<marks>` this message contains | Make sure they are in your translation |

You may add a column of your own (for example **Note**) to ask a question or say that an English sentence is hard to translate. PrismQDA ignores columns it does not know. Please send the questions back.

The most-used screens come first in the file: the welcome page, new project, import, the ribbon, and settings. A translation of the first few hundred rows is already useful, and anything you leave empty is shown in English, so you can send a partly finished file and finish it later.

## Four things you must keep

**1. Names in braces: `{name}`.** PrismQDA fills these in when it shows the message. Keep them exactly as written, and put them wherever your language needs them.

> English: `Delete {name}?`
> Translation (French): `Supprimer {name} ?`

Do not translate what is inside the braces. A translation that changes or loses a name is left out when the file is added, and the English is shown instead; the program lists these, so nothing is silently wrong.

**2. Marks: `<kbd>Enter</kbd>`, `<strong>...</strong>`.** These make part of the text look different (a key, bold). Keep the marks around the words they mark, and translate the words inside. You can move the marked words.

> English: `Press <kbd>Enter</kbd> to code {name}.`
> Translation (Spanish): `Pulse <kbd>Intro</kbd> para codificar {name}.`

**3. Numbers: the forms.** A message that shows a number appears on more than one row, one for each way your language changes the words for numbers. The **Form** column says which:

- English has two: `one` ("1 code") and `other` ("2 codes").
- Your language may need more: for example `few` (2 to 4 in some Slavic languages), `many`, `two`, `zero` (Arabic uses all six). The file lists exactly the rows your language needs.
- A language with no plural (Chinese, Japanese) has one row, `other`.
- Write the whole text in each row, including `{count}`, which is replaced by the number.

**4. Single braces.** If a message must show a real `{` or `}`, it is written `{{` or `}}`. Keep that as it is.

## Words that stay as they are

- **PrismQDA** is the name of the program.
- Names of file kinds (PDF, DOCX, CSV) and file endings (.pqda).
- Key names such as Ctrl, Shift, Enter, and Esc are the names printed on keys. If keyboards in your country print them in your language, use that; otherwise keep them.

## Sentences made of pieces

A few messages are pieces of a longer sentence that the program joins together, for example `the code is {text}{list}` or `, merged into another code`. They were written for English word order. Translate each piece so that the pieces read well in your order, and tell us (in the **Note** column) when that is impossible. We will rewrite those messages as whole sentences.

## Save and return the file

- **Excel**: Save As, then **CSV UTF-8 (Comma delimited)**. The plain "CSV" choice loses letters with accents and letters of other scripts. (PrismQDA warns you if that happened.) Semicolons or tabs instead of commas are also fine.
- **Google Sheets**: File, Download, Comma-separated values (.csv).
- **A JSON file**: keep it as UTF-8.

Return the file to the person who gave it to you. They add it in PrismQDA with **Settings > Language > Add a translation**, which tells them how many messages were translated and lists anything left out and why.

## If a machine made the first draft

You may use a translation program for a first draft. If you do, read **every** message and correct it, and tell the person who adds the file how it was made. PrismQDA keeps one of three labels with each translation: a person translated it; a person reviewed a machine translation; or a machine translated it and nobody has checked it. People who use the program, and who may write about their methods, can see which it is.

## Glossary of the main words

| English | What it means in PrismQDA |
| --- | --- |
| **Project** | One study, kept in one file on the researcher's computer |
| **Document**, **source** | A piece of data: an interview, a set of survey answers, an article, a picture |
| **Case** | A person, a site, or a group that documents belong to (an interviewee; a school) |
| **Code** | A short name for an idea in the data ("Trust", "Cost") |
| **Codebook** | The whole list of codes, with what each one means |
| **Sub-code**, **theme** | A code under another code; a code that groups other codes |
| **Coding** (noun) | One code put on one stretch of text (or one area of a picture) |
| **To code** (verb) | To put a code on a passage |
| **Coder** | The person (or program) that made a coding. More than one coder can code the same data |
| **Passage**, **segment**, **excerpt** | A stretch of text (or an area of a picture) that is, or can be, coded |
| **Unit** | A part of a document that can be coded whole: a sentence, a paragraph, one speaker's turn, one survey answer |
| **Memo** | A note the researcher writes about the data, a code, or the project |
| **In vivo** | A code named with the participant's own words |
| **Retrieved Segments** | The table of all passages coded with the codes you choose |
| **Query** | A question asked of the coding ("passages coded Cost and also Trust") |
| **Code Matrix**, **Crosstab**, **Code Relations**, **Concept Map** | Tables and a diagram that show how codes and cases relate |
| **Auto Code** | Finding passages by a rule and suggesting a code for them, to be reviewed |
| **Review queue** | Suggestions waiting for the researcher to accept or reject |
| **AI prompt packet** | Text the researcher copies to a chatbot of their own choosing. PrismQDA never sends anything anywhere |
| **Anchor example**, **counterexample** | A passage that clearly shows what a code means; a passage that looks like it but does not |
| **Mutually exclusive group** | Codes of which only one may be put on the same passage |
| **Rated code** | A code that also carries a rating from 1 to 5 |
| **Backup** | A dated copy of the whole project, kept beside it |
| **Sample project** | A made-up study to practice on |

If a word is not in this list and you are unsure what it means in context, ask. A wrong guess in a research tool costs a researcher time.
