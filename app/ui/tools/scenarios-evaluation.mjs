// Two things that come in from the other programs of the family (development list items 4 and 5): an evaluation kept in PrismStatistics, a
// `.pstat` file with ratings and open comments, coded here beside the ratings; and the exchange file of Prism Reference Manager, with
// sources and the excerpts taken from them. All the data is made up.

import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

/** A zip archive of stored (uncompressed) entries, which every reader opens; enough for a `.pstat` file. */
function zipStored(entries) {
  const chunks = [];
  const central = [];
  let offset = 0;
  for (const [name, text] of entries) {
    const nameBytes = Buffer.from(name, 'utf8');
    const data = Buffer.from(text, 'utf8');
    const crc = zlib.crc32(data) >>> 0;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x21, 12); // the date: 1 January 1980
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    chunks.push(local, nameBytes, data);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50, 0);
    entry.writeUInt16LE(20, 4);
    entry.writeUInt16LE(20, 6);
    entry.writeUInt16LE(0x21, 14);
    entry.writeUInt32LE(crc, 16);
    entry.writeUInt32LE(data.length, 20);
    entry.writeUInt32LE(data.length, 24);
    entry.writeUInt16LE(nameBytes.length, 28);
    entry.writeUInt32LE(offset, 42);
    central.push(entry, nameBytes);
    offset += 30 + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...chunks, directory, end]);
}

const EVALUATION = {
  format: 'PrismStatistics project',
  version: 1,
  app_version: '0.1.1',
  dataset: {
    name: 'Evaluation of a workshop',
    variables: [
      { id: 1, name: 'ev_participant', label: 'Participant', measure: 'nominal', value_labels: [], missing_values: [] },
      { id: 2, name: 'ev_rating', label: 'Overall rating', measure: 'ordinal', value_labels: [{ value: 1, label: 'Poor' }, { value: 5, label: 'Excellent' }], missing_values: [9] },
      { id: 3, name: 'ev_hours', label: '', measure: 'scale', value_labels: [], missing_values: [] },
      { id: 4, name: 'ev_comment', label: 'What worked well?', measure: 'nominal', value_labels: [], missing_values: [] },
    ],
    columns: [
      { type: 'text', values: ['EV1', 'EV2', 'EV3', 'EV4'] },
      { type: 'numeric', values: [5, 2, 9, 4] },
      { type: 'numeric', values: [3.5, 1, null, 2] },
      {
        type: 'text',
        values: [
          'The role plays helped me practise the hard conversations.',
          'The pace was too fast and I got lost in the second half of the day.',
          null,
          'The facilitator gave clear examples and the role plays were useful.',
        ],
      },
    ],
    next_id: 5,
  },
  source: null,
  operations: [],
  results: [],
};

const EXCHANGE = {
  manifest: { format: 'prism-family-exchange', version: '1.0', producer: 'Prism Reference Manager', producer_version: '0.1.2', created: '2026-11-15T10:00:00Z', project: { id: 'p1', title: 'A review of waiting' } },
  sources: [
    {
      id: 'ex-s1',
      citekey: 'ex_hernandez2021',
      csl: { id: 'ex-s1', type: 'article-journal', title: 'Bus reliability and daily stress', author: [{ family: 'Hernandez', given: 'Maria' }], issued: { 'date-parts': [[2021]] }, 'container-title': 'Journal of Transit', abstract: 'Late buses raise stress.' },
    },
    { id: 'ex-s2', citekey: 'ex_okafor2019', csl: { id: 'ex-s2', type: 'book', title: 'Waiting', author: [{ family: 'Okafor', given: 'Ada' }], issued: { 'date-parts': [[2019]] } } },
  ],
  excerpts: [
    { id: 'ex-e1', type: 'quotation', source_ids: ['ex-s1'], exact_text: 'Riders waited a mean of 14 minutes.', exact_text_edited: false, user_text: 'Compare with our data.', locator: { label: 'page', value: '12' }, anchor_state: 'resolved', origin: 'user', annotation_id: 'a-1' },
    { id: 'ex-e2', type: 'summary', source_ids: ['ex-s2'], exact_text: '', user_text: 'Waiting is a social experience.', locator: { label: '', value: '' }, anchor_state: 'resolved', origin: 'user', annotation_id: '' },
  ],
};

export async function runEvaluation(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const importDialog = () => page.getByRole('main', { name: 'Import Data' });
  async function openImport(file) {
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(file);
    return dialog;
  }
  const finish = async () => {
    const done = page.getByTestId('import-finished');
    await done.waitFor();
    await done.getByRole('button', { name: 'Done' }).click();
    await done.waitFor({ state: 'hidden' });
  };

  await S('an evaluation kept in PrismStatistics opens like a spreadsheet: the respondent and the comments are guessed, and the ratings keep their labels', async () => {
    const file = path.join(workDir, 'evaluation.pstat');
    fs.writeFileSync(file, zipStored([['project.json', JSON.stringify(EVALUATION)]]));
    const dialog = await openImport(file);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'Tell PrismQDA what each column is' }).waitFor();
    const role = (col) => dialog.getByLabel(`What column ${col} is for`);
    h.equal(await role('ev_participant').inputValue(), 'id', 'the participant is guessed as the person ID');
    h.equal(await role('ev_comment').inputValue(), 'text', 'the comment is guessed as open-ended text');
    h.equal(await role('ev_rating').inputValue(), 'attribute', 'the rating describes the person');
    h.equal(await dialog.getByLabel('Kind of values in column ev_rating').inputValue(), 'ordinal', 'the file says the rating is ordered, and the window believes it');
    await axe('the import step for a PrismStatistics project');
    await dialog.getByRole('button', { name: 'Show what will be imported' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    h.check(/4 cases will be made/.test(await dialog.textContent()), 'four cases, one for each participant');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await finish();
    const cases = (await rpc('case.list')).map((c) => c.name);
    for (const name of ['EV1', 'EV2', 'EV3', 'EV4']) h.check(cases.includes(name), `${name} is a case`);
    const variables = await rpc('stats.variables');
    const rating = variables.variables.find((v) => v.key === 'attr:ev_rating');
    h.check(rating !== undefined, 'the rating is a variable of the cases');
    h.equal(rating.valid, 3, 'the file marks 9 as no answer, and it is set aside');
    h.check(rating.categories.some((c) => c.label === 'Excellent'), 'the file\'s value labels came with the ratings');
  });

  await S('the exchange file of Prism Reference Manager is told by what is in it, and each source comes in as a document and a case', async () => {
    const file = path.join(workDir, 'review-notes.json');
    fs.writeFileSync(file, JSON.stringify(EXCHANGE));
    const dialog = await openImport(file);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    await dialog.getByRole('heading', { name: 'What will come in' }).waitFor();
    const text = await dialog.textContent();
    h.check(/Library sources \(2\)|Library source/.test(text), `the type says what it is: ${text.slice(text.indexOf('What will come in'), text.indexOf('What will come in') + 240)}`);
    h.check(/Hernandez/.test(text) && /Okafor/.test(text), 'both sources are named');
    await axe('the import review of a Reference Manager exchange file');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await finish();
    const names = (await rpc('source.list')).map((s) => s.name);
    h.check(names.some((n) => /Hernandez 2021/.test(n)), 'the first source is a document');
    const first = (await rpc('source.list')).find((s) => /Hernandez 2021/.test(s.name));
    const body = (await rpc('source.get', { id: first.id })).text;
    h.check(body.includes('Quotation, page 12') && body.includes('“Riders waited a mean of 14 minutes.”'), 'its excerpt is in it, with its place');
    const variables = await rpc('stats.variables');
    for (const key of ['attr:Citation key', 'attr:Library source id', 'attr:Excerpts taken']) h.check(variables.variables.some((v) => v.key === key), `${key} is a variable of the cases`);
  });
}
