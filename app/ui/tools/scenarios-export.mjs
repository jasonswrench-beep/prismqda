// Exporting the codebook and coded passages, and importing a whole folder.

import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

/** One file out of a zip (a Word file is one), read from its central directory; null when it is not there. */
export function unzipEntry(file, name) {
  const buf = fs.readFileSync(file);
  const end = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  if (end < 0) return null;
  const count = buf.readUInt16LE(end + 10);
  let at = buf.readUInt32LE(end + 16);
  for (let i = 0; i < count && buf.readUInt32LE(at) === 0x02014b50; i++) {
    const method = buf.readUInt16LE(at + 10);
    const size = buf.readUInt32LE(at + 20);
    const nameLength = buf.readUInt16LE(at + 28);
    const extraLength = buf.readUInt16LE(at + 30);
    const commentLength = buf.readUInt16LE(at + 32);
    const local = buf.readUInt32LE(at + 42);
    if (buf.toString('utf8', at + 46, at + 46 + nameLength) === name) {
      const start = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28);
      const data = buf.subarray(start, start + size);
      return method === 0 ? data : zlib.inflateRawSync(data);
    }
    at += 46 + nameLength + extraLength + commentLength;
  }
  return null;
}

export async function runExport(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe, mainTab } = api;

  const header = () => page.getByRole('toolbar', { name: 'Commands' });
  const dialog = (name) => page.getByRole('dialog', { name, exact: true });
  const exportDir = path.join(workDir, 'exports');
  fs.mkdirSync(exportDir, { recursive: true });

  const openExport = async () => {
    await mainTab('Coding').click();
    // Export is on the Report tab of the ribbon.
    await page.getByRole('tab', { name: 'Report', exact: true }).click();
    await header().getByRole('button', { name: 'Export', exact: true }).click();
    const d = dialog('Export');
    await d.waitFor();
    return d;
  };
  /** Waits until the engine has written the file (the status line still shows the previous save until then). */
  const waitFile = async (file, ms = 8000) => {
    const t0 = Date.now();
    while (Date.now() - t0 < ms) {
      if (fs.existsSync(file)) return true;
      await api.sleep(50);
    }
    return false;
  };
  const saveTo = async (d, file) => {
    await d.getByRole('textbox', { name: 'Where to save it' }).fill(file);
    await d.getByRole('button', { name: 'Save the file', exact: true }).click();
  };

  await S('the Export dialog saves the codebook as Excel, Word, QDC, and CSV', async () => {
    const d = await openExport();
    await axe('the export dialog');
    const codes = await rpc('code.list');
    h.check(codes.length > 0, 'the project has codes to export');
    const results = {};
    for (const [label, ending] of [['Excel', 'xlsx'], ['Word', 'docx'], ['QDC codebook', 'qdc'], ['CSV', 'csv']]) {
      await d.getByText(label, { exact: true }).click();
      const file = path.join(exportDir, `codebook-${ending}`);
      await saveTo(d, file);
      const real = `${file}.${ending}`;
      h.check(await waitFile(real), `${real} was written (the ending was added)`);
      results[ending] = fs.readFileSync(real);
    }
    h.equal(results.xlsx.subarray(0, 2).toString('latin1'), 'PK', 'the Excel file is a zip package');
    h.equal(results.docx.subarray(0, 2).toString('latin1'), 'PK', 'the Word file is a zip package');
    const qdc = results.qdc.toString('utf8');
    h.check(qdc.includes('<CodeBook xmlns="urn:QDA-XML:codebook:1.0"') && qdc.includes(`name="${codes[0].name}"`), 'the QDC file names the codes');
    const csv = results.csv.toString('utf8');
    h.check(csv.startsWith('﻿Code,Name,Parent'), 'the CSV starts with a byte order mark and its header');
    const said = page.getByText(/^Saved .*codebook-csv\.csv/).first();
    await said.waitFor({ timeout: 5000 }).catch(() => {});
    h.check(await said.isVisible(), 'the dialog says where the file went');
  });

  await S('an export never replaces a file that is already there', async () => {
    const d = dialog('Export');
    const file = path.join(exportDir, 'codebook-csv.csv');
    const before = fs.readFileSync(file);
    await saveTo(d, file);
    await d.getByRole('alert').waitFor();
    h.check(/already exists/.test(await d.getByRole('alert').textContent()), 'a plain message says the file exists');
    h.check(fs.readFileSync(file).equals(before), 'the file is unchanged');
    await axe('the export dialog with an error');
  });

  await S('coded passages can be exported for one code, as Word and Excel', async () => {
    const d = dialog('Export');
    await d.getByText('The coded passages', { exact: true }).click();
    h.check((await d.getByRole('radio', { name: /^CSV/ }).count()) === 1, 'the file types change to the ones that suit passages');
    h.check((await d.getByRole('radio', { name: /^QDC/ }).count()) === 0, 'QDC is only for codebooks');
    const codes = await rpc('code.list');
    const used = codes.find((c) => c.usage > 0 && c.status === 'active');
    h.check(!!used, 'some code has been used');
    await d.locator('#exp-code').selectOption({ label: used.name });
    await d.getByText('Word', { exact: true }).click();
    const word = path.join(exportDir, 'passages');
    await saveTo(d, word);
    h.check(await waitFile(`${word}.docx`), 'the Word file was written');
    h.check(fs.statSync(`${word}.docx`).size > 1000, 'the Word file has content');
    await d.getByText('Excel', { exact: true }).click();
    await saveTo(d, word);
    h.check(await waitFile(`${word}.xlsx`), 'the Excel file was written');
    h.check(fs.statSync(`${word}.xlsx`).size > 1000, 'the Excel file has content');
    const listed = await rpc('segments.list', { code_ids: [used.id] });
    h.check(listed.total > 0, 'the engine has passages for that code');
    await d.getByRole('button', { name: 'Close', exact: true }).click();
    await d.waitFor({ state: 'hidden' });
  });

  await S('a whole folder can be imported, and what went wrong is listed', async () => {
    const folder = path.join(workDir, 'folder-import');
    fs.mkdirSync(path.join(folder, 'wave 2'), { recursive: true });
    fs.writeFileSync(path.join(folder, 'one.txt'), 'First made-up interview. It was fine.\n\nA second paragraph.', 'utf8');
    fs.writeFileSync(path.join(folder, 'wave 2', 'two.md'), '# Two\n\nAnother made-up interview.', 'utf8');
    fs.writeFileSync(path.join(folder, 'broken.docx'), 'this is not a Word file', 'utf8');
    fs.writeFileSync(path.join(folder, 'scores.csv'), 'id,text\n1,hello\n', 'utf8');
    const before = (await rpc('source.list')).length;

    await mainTab('Coding').click();
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
    await header().getByRole('button', { name: 'Add Documents', exact: true }).click();
    const imp = page.getByRole('main', { name: 'Import Data' });
    await imp.waitFor();
    await imp.getByRole('button', { name: 'Import a whole folder instead', exact: true }).click();
    const d = dialog('Import a whole folder');
    await d.waitFor();
    await axe('the folder import dialog');
    const facts = d.getByTestId('folder-picture-facts');
    h.equal(await facts.getByRole('checkbox').count(), 3, 'the dialog offers the facts a camera writes into a photograph: when, with what, and where');
    h.equal(await facts.locator('input:checked').count(), 0, 'and none is kept unless it is ticked');
    await d.getByRole('textbox', { name: 'Folder to import' }).fill(folder);
    await d.getByRole('textbox', { name: 'Put the sources in a folder called' }).fill('Folder test');
    await d.getByRole('button', { name: 'Import the folder', exact: true }).click();
    const done = dialog('Folder import finished');
    await done.waitFor();
    const text = await done.textContent();
    h.check(/Imported 2 sources/.test(text), `two sources were imported: ${text.slice(0, 120)}`);
    h.check(/1 file could not be read/.test(text) && /broken\.docx/.test(text), 'the broken file is named');
    h.check(/1 file skipped/.test(text) && /scores\.csv/.test(text), 'the spreadsheet is listed as skipped');
    await axe('the folder import result');
    const after = await rpc('source.list');
    h.equal(after.length, before + 2, 'two sources were added');
    h.check(after.some((s) => s.folder === 'Folder test/wave 2'), 'the subfolder became a folder beneath the one named');
    await done.getByRole('button', { name: 'Done', exact: true }).click();
    await done.waitFor({ state: 'hidden' });
  });
  await S('a report is built from the project records, with the sections chosen, and saved as a Word file', async () => {
    await mainTab('Coding').click();
    await page.getByRole('tab', { name: 'Report', exact: true }).click();
    await header().getByRole('button', { name: 'Report Builder', exact: true }).click();
    await page.getByTestId('build-report').click();
    const d = dialog('Export');
    await d.waitFor();
    h.check(await d.getByRole('radio', { name: /A report/ }).isChecked(), 'the report is chosen when it is opened from the report page');
    h.equal(await d.getByRole('radio', { name: /^(Excel|Word|CSV|QDC)/ }).count(), 0, 'and there is only one kind of file, so no choice of file type');
    const sections = d.getByTestId('report-sections');
    h.equal(await sections.getByRole('checkbox').count(), 13, 'thirteen sections are offered');
    h.equal(await sections.locator('input:checked').count(), 6, 'six are chosen to begin with');
    h.equal(await sections.getByTestId('report-section-passages').locator('input').isChecked(), false, 'the coded passages, which can be long, are not');
    await axe('the export dialog set to a report');

    // Nothing chosen is refused in words, and nothing is written.
    const file = path.join(exportDir, 'check-report.docx');
    for (const id of ['study', 'documents', 'codebook', 'agreement', 'ai_use', 'methods']) await sections.getByTestId(`report-section-${id}`).click();
    await saveTo(d, file);
    await d.getByRole('alert').waitFor();
    h.check(/at least one section/.test(await d.getByRole('alert').textContent()), 'a report with no section is refused in words');
    h.equal(fs.existsSync(file), false, 'and nothing is written');

    // Choose the memos, the methods appendix, and the timeline, and give the report a title.
    for (const id of ['memos', 'methods', 'timeline']) await sections.getByTestId(`report-section-${id}`).click();
    await d.getByTestId('report-title').fill('A check report');
    await saveTo(d, file);
    h.check(await waitFile(file), 'the Word file was written');
    const xml = unzipEntry(file, 'word/document.xml')?.toString('utf8') ?? '';
    h.check(xml.includes('A check report'), 'the title is the one given');
    h.check(xml.includes('Methods appendix (draft)') && xml.includes('Decision timeline') && xml.includes('>Memos<'), 'the sections chosen are in it');
    h.check(!xml.includes('>Codebook<') && !xml.includes('>Documents<'), 'and the ones that were not chosen are not');
    h.check(xml.includes('Imported '), 'the timeline tells what was imported');
    await d.getByRole('button', { name: 'Close', exact: true }).click();
    await d.waitFor({ state: 'hidden' });
  });

  await S('the whole project is exported as QDPX with a report, and a QDPX file is brought in as a new project that is listed beside it', async () => {
    const d = await openExport();
    await d.getByText('The whole project', { exact: true }).click();
    h.check(await d.getByTestId('qdpx-note').isVisible(), 'it says the documents go in whole and the file is not encrypted');
    const file = path.join(exportDir, 'whole-project');
    await saveTo(d, file);
    h.check(await waitFile(`${file}.qdpx`), 'the QDPX file was written, with its ending added');
    await d.getByTestId('qdpx-report').waitFor();
    const codes = (await rpc('code.list')).filter((c) => c.status !== 'merged');
    const carried = (await d.getByTestId('qdpx-carried').locator('li').allTextContents()).join(' ; ');
    h.check(new RegExp(`${codes.length} codes?\\b`).test(carried ?? ''), `the report counts the ${codes.length} codes it carries: ${carried}`);
    h.check((await d.getByTestId('qdpx-left-out').locator('li').count()) >= 2, 'and lists what the format cannot hold');
    await axe('the export dialog with the QDPX report');
    // the package is what the standard describes
    const qde = unzipEntry(`${file}.qdpx`, 'project.qde')?.toString('utf8') ?? '';
    h.check(qde.includes('<Project xmlns="urn:QDA-XML:project:1.0"') && qde.includes('<TextSource '), 'it holds project.qde in the standard\u2019s namespace, with the documents');
    h.check(qde.includes('startPosition=') && qde.includes('<CodeRef targetGUID='), 'and the codings, as selections that refer to codes');
    // a file that is there is not replaced
    await saveTo(d, file);
    await d.getByRole('alert').waitFor();
    h.check(/already exists/.test(await d.getByRole('alert').textContent()), 'a file that is already there is not replaced');
    await d.getByRole('button', { name: 'Cancel', exact: true }).click();
    await d.waitFor({ state: 'hidden' });

    // Import QDPX from the command box: a file that is not a QDPX file is refused in words
    await page.keyboard.press('Control+k');
    const box = page.getByRole('dialog', { name: 'Search or run a command' });
    await box.waitFor();
    await page.keyboard.type('qdpx');
    // The command box shows a command by what it does, which is its title.
    await box.getByRole('option', { name: /Bring in a whole project from another qualitative analysis program/ }).click();
    const imp = page.getByRole('dialog', { name: 'Import QDPX', exact: true });
    await imp.waitFor();
    await axe('the Import QDPX dialog');
    await imp.getByTestId('qdpx-file').fill(path.join(exportDir, 'codebook-csv.csv'));
    await imp.getByTestId('qdpx-import').click();
    await imp.getByRole('alert').waitFor();
    h.check(/not a QDPX file/.test(await imp.getByRole('alert').textContent()), 'a file that is not a ZIP of a project is refused in words');
    h.equal(fs.existsSync(path.join(exportDir, 'codebook-csv.pqda')), false, 'and no project is left behind');

    // the file just made comes in as a new project, saved beside it, and the open project is not changed
    const open = await rpc('project.info');
    await imp.getByTestId('qdpx-file').fill(`${file}.qdpx`);
    await imp.getByTestId('qdpx-import').click();
    await imp.getByTestId('qdpx-report').waitFor();
    const came = (await imp.getByTestId('qdpx-carried').locator('li').allTextContents()).join(' ; ');
    h.check(new RegExp(`${codes.length} codes?\\b`).test(came ?? ''), `the same codes come in: ${came}`);
    h.check(fs.existsSync(`${file}.pqda`), 'the new project is saved beside the QDPX file, with its name');
    h.equal((await rpc('project.info')).path, open.path, 'the project that was open is still the open one');
    await axe('the Import QDPX dialog with its report');
    await imp.getByRole('button', { name: 'Close', exact: true }).click();
    await imp.waitFor({ state: 'hidden' });
  });
}
