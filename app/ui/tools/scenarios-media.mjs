// Recordings: importing a recording through the captions beside it, the player above the text, the text and the recording following each
// other, a recording that moved, and taking a link away. The recording is a short silent WAV the check makes itself, and the check never
// presses Play: nothing here starts a recording, so nothing can be heard.

import fs from 'node:fs';
import path from 'node:path';

/** Six seconds of silence as an 8-bit mono WAV, small enough to load at once. */
function silentWav(seconds = 6, rate = 8000) {
  const samples = seconds * rate;
  const buf = Buffer.alloc(44 + samples, 0x80);
  buf.write('RIFF', 0);
  buf.writeUInt32LE(36 + samples, 4);
  buf.write('WAVE', 8);
  buf.write('fmt ', 12);
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(rate, 24);
  buf.writeUInt32LE(rate, 28);
  buf.writeUInt16LE(1, 32);
  buf.writeUInt16LE(8, 34);
  buf.write('data', 36);
  buf.writeUInt32LE(samples, 40);
  return buf;
}

const CAPTIONS =
  'WEBVTT\n\n1\n00:00:00.500 --> 00:00:02.000\n<v Ann>Hello there.\n\n2\n00:00:02.500 --> 00:00:04.000\n<v Bo>Fine, thanks.\n\n3\n00:00:04.500 --> 00:00:06.000\n<v Ann>Good to hear.\n';

export async function runMedia(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const importDialog = () => page.getByRole('main', { name: 'Import Data' });
  const element = () => page.getByTestId('media-element');
  const state = () => page.evaluate(() => {
    const a = document.querySelector('[data-testid=media-element]');
    return a ? { ready: a.readyState, duration: a.duration, paused: a.paused, time: a.currentTime, tag: a.tagName } : null;
  });
  const waitLoaded = () => page.waitForFunction(() => {
    const a = document.querySelector('[data-testid=media-element]');
    return !!a && a.readyState >= 1 && a.duration > 5;
  }, null, { timeout: 8000 });
  const seek = (seconds) => page.evaluate((s) => { document.querySelector('[data-testid=media-element]').currentTime = s; }, seconds);
  /** The tools under the player are folded away until asked for. */
  const openTools = async () => {
    const tools = page.getByTestId('media-more');
    if (!(await tools.evaluate((el) => el.open))) await tools.locator('summary').click();
  };
  const documentRow = (name) => page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: new RegExp(name) }).first();

  const wav = path.join(workDir, 'media-talk.wav');
  const vtt = path.join(workDir, 'media-talk.vtt');
  let source;

  await S('a recording is imported through the captions beside it, and the page says so before anything is added', async () => {
    fs.writeFileSync(wav, silentWav());
    fs.writeFileSync(vtt, CAPTIONS, 'utf8');
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(wav);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const note = dialog.getByTestId('recording-note');
    await note.waitFor();
    const said = await note.innerText();
    h.check(/media-talk\.wav \(audio\)/.test(said) && /read from media-talk\.vtt/.test(said) && /does not copy it into the project/.test(said), `the recording and its captions are named: ${said.replace(/\s+/g, ' ').slice(0, 200)}`);
    h.check(/Transcript/.test(await dialog.getByTestId('import-queue').innerText()), 'what comes in is a transcript');
    h.equal((await rpc('source.list')).filter((s) => s.name === 'media-talk').length, 0, 'checking the file added nothing yet');
    await axe('the Import Data page with a recording');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await page.getByTestId('import-finished').waitFor();
    source = (await rpc('source.list')).find((s) => s.name === 'media-talk');
    const row = await rpc('media.get', { source_id: source.id });
    h.equal([row.kind, row.found, row.unchanged, row.duration_ms, row.file_name], ['audio', true, true, 6000, 'media-talk.wav'], 'the recording is linked, found, and as long as the last turn says');
    const times = (await rpc('unit.list', { source_id: source.id })).map((u) => [u.time_start_ms, u.time_end_ms]);
    h.equal(times, [[500, 2000], [2500, 4000], [4500, 6000]], 'each turn knows when it is spoken');
    await page.getByTestId('import-finished').getByRole('button', { name: 'Done' }).click();
  });

  await S('the player is above the text, loads the recording, and does not start by itself', async () => {
    await page.getByTestId('media-player').waitFor();
    await waitLoaded();
    const s = await state();
    h.equal([s.tag, s.paused, Math.abs(s.duration - 6) < 0.3], ['AUDIO', true, true], 'it is an audio player, six seconds long, and paused');
    h.equal(await page.getByTestId('media-name').innerText(), 'media-talk.wav', 'it says which recording it is');
    h.check(/Audio, \d+ KB/.test(await page.getByTestId('media-player').innerText()), 'and what kind and how large');
    await page.screenshot({ path: `${ctx.shots}/media-player.png`, fullPage: true });
    await axe('the reader with a recording');
  });

  await S('the turn being spoken follows the recording, and the recording moves to a turn without starting', async () => {
    await seek(3);
    await page.waitForFunction(() => /Fine, thanks\./.test(document.querySelector('[data-testid=media-now]')?.textContent ?? ''), null, { timeout: 5000 });
    h.check(/^Now: Bo: Fine, thanks\./.test(await page.getByTestId('media-now').innerText()), 'at three seconds the speaker and words of that turn are told');
    await seek(0.2);
    await page.waitForFunction(() => /Before the first turn/.test(document.querySelector('[data-testid=media-now]')?.textContent ?? ''), null, { timeout: 5000 });
    h.check(true, 'before the first turn it says so');

    const units = await rpc('unit.list', { source_id: source.id });
    await openTools();
    await page.getByLabel('Move the recording to a turn').selectOption(String(units[2].id));
    await page.waitForFunction(() => Math.abs(document.querySelector('[data-testid=media-element]').currentTime - 4.5) < 0.05, null, { timeout: 5000 });
    h.equal((await state()).paused, true, 'choosing a turn moves the recording there and leaves it paused');
    await page.waitForFunction(() => /Good to hear\./.test(document.querySelector('[data-testid=media-now]')?.textContent ?? ''), null, { timeout: 5000 });

    await page.getByTestId('media-go-words').click();
    await page.waitForFunction(() => /Good to hear/.test(window.getSelection()?.toString() ?? ''), null, { timeout: 5000 });
    h.check(true, 'the words playing now can be found in the text: they are selected');
    h.equal(await page.getByTestId('media-follow').getByRole('checkbox').isChecked(), false, 'the text does not scroll after the recording unless asked to');
    await axe('the reader with the recording moved to a turn');
  });

  await S('a recording that moved is told, and linking it again from its new place is the way back', async () => {
    const moved = path.join(workDir, 'media-talk-moved.wav');
    fs.renameSync(wav, moved);
    await page.reload();
    await documentRow('media-talk').click();
    const missing = page.getByTestId('media-missing');
    await missing.waitFor();
    h.check(/is not at .*media-talk\.wav any more\. Link it again/.test(await missing.innerText()), 'it says the recording is not where it was');
    h.equal(await element().count(), 0, 'and there is no player to give a wrong answer');
    await axe('the reader with a recording that moved');
    await page.getByLabel('Full path of the recording').fill(moved);
    await page.getByTestId('media-link').click();
    await element().waitFor();
    await waitLoaded();
    h.equal((await rpc('media.get', { source_id: source.id })).file_name, 'media-talk-moved.wav', 'the link now names the new place');
    h.equal(await page.getByTestId('media-missing').count(), 0, 'and nothing says it is missing');
  });

  await S('a link is taken away with a button, the recording is left alone, and undo links it again', async () => {
    await openTools();
    await page.getByTestId('media-unlink').click();
    await page.getByText(/Recording: none linked/).waitFor();
    h.equal(await rpc('media.get', { source_id: source.id }), null, 'the document has no recording linked');
    h.check(fs.existsSync(path.join(workDir, 'media-talk-moved.wav')), 'the recording itself was not touched');
    await page.getByText(/Recording: none linked/).click();
    await page.getByLabel('Full path of the recording').waitFor();
    await axe('a transcript with no recording linked');
    await rpc('undo');
    await page.reload();
    await documentRow('media-talk').click();
    await waitLoaded();
    h.equal((await rpc('media.get', { source_id: source.id })).file_name, 'media-talk-moved.wav', 'undo links the recording again');
  });

  await S('a recording with no captions beside it is not refused: the page says it comes in as a document of its own', async () => {
    const lonely = path.join(workDir, 'media-lonely.mp3');
    fs.writeFileSync(lonely, Buffer.alloc(64, 1));
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(lonely);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const note = dialog.getByTestId('recording-alone-note');
    await note.waitFor();
    h.check(/does not turn speech into text/.test(await note.innerText()) && /document of its own/.test(await note.innerText()), 'the page says PrismQDA does not turn speech into text, and that the recording is coded by stretches of time');
    h.equal((await rpc('source.list')).filter((s) => /lonely/.test(s.name)).length, 0, 'nothing was imported yet');
    await axe('the Import Data page with a recording that has no captions');
    await page.getByRole('button', { name: 'Back to the coding workspace' }).first().click();
  });
}
