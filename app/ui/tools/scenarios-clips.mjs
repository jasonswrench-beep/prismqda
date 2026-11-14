// Coding a stretch of a recording: a recording with no captions comes in as a document of its own, a stretch is marked with the player's
// time (moved without playing) or typed, given a code through the code box, shown on the timeline and in the lists, moved, played from a
// button's request (never here), merged into queries, and kept when the project is opened again. The recording is a silent WAV the check
// makes itself, and the check NEVER presses Play or any button that plays: nothing here starts a recording, so nothing can be heard.

import fs from 'node:fs';
import path from 'node:path';

/** Twelve seconds of silence as an 8-bit mono WAV, small enough to load at once. */
function silentWav(seconds = 12, rate = 8000) {
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

/** Twelve seconds of mostly silence with one loud second (4 to 5 seconds in), as an 8-bit mono WAV. It is only ever read and drawn, never played. */
function toneWav(seconds = 12, rate = 8000, at = 4) {
  const buf = silentWav(seconds, rate);
  for (let i = at * rate; i < (at + 1) * rate; i++) buf[44 + i] = (i >> 1) % 2 === 0 ? 0xe0 : 0x20;
  return buf;
}

/** What a WAV file holds, read from its header: the channels, the rate, the bits, the frames, and the loudest sample (as a share of full scale). */
function readWav(file) {
  const b = fs.readFileSync(file);
  const word = (at) => b.toString('latin1', at, at + 4);
  const frames = b.readUInt32LE(40) / (b.readUInt16LE(22) * (b.readUInt16LE(34) / 8));
  let peak = 0;
  for (let at = 44; at + 1 < b.length; at += 2) peak = Math.max(peak, Math.abs(b.readInt16LE(at)));
  return { riff: word(0), wave: word(8), channels: b.readUInt16LE(22), rate: b.readUInt32LE(24), bits: b.readUInt16LE(34), frames, seconds: frames / b.readUInt32LE(24), peak: peak / 32768, size: b.length, declared: b.readUInt32LE(4) + 8 };
}

export async function runClips(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const importDialog = () => page.getByRole('main', { name: 'Import Data' });
  const documentRow = (name) => page.getByRole('treegrid', { name: 'Documents' }).getByRole('row', { name: new RegExp(name) }).first();
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  const state = () => page.evaluate(() => {
    const a = document.querySelector('[data-testid=media-element]');
    return a ? { ready: a.readyState, duration: a.duration, paused: a.paused, time: a.currentTime } : null;
  });
  const waitLoaded = () => page.waitForFunction(() => {
    const a = document.querySelector('[data-testid=media-element]');
    return !!a && a.readyState >= 1 && a.duration > 10;
  }, null, { timeout: 8000 });
  /** Moves the recording to a time, without playing it. */
  const seek = async (seconds) => {
    await page.evaluate((s) => { document.querySelector('[data-testid=media-element]').currentTime = s; }, seconds);
    await page.waitForFunction((s) => Math.abs(document.querySelector('[data-testid=media-element]').currentTime - s) < 0.05, seconds, { timeout: 5000 });
    // The readout under the player follows the recording.
    await page.waitForFunction((s) => {
      const el = document.querySelector('[data-testid=clip-now]');
      return !!el && el.textContent.includes(`${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`);
    }, seconds, { timeout: 5000 });
  };
  const regions = () => rpc('region.list', { source_id: source.id });
  const code = async (name) => {
    const box = page.getByRole('dialog', { name: 'Apply a code' });
    await page.getByTestId('clip-apply').click();
    await box.waitFor();
    await box.getByRole('combobox').fill(name);
    await page.keyboard.press('Enter');
    await box.waitFor({ state: 'detached' });
  };

  const wav = path.join(workDir, 'clips-field.wav');
  let source;
  let humor;
  let worry;

  await S('a recording with no captions beside it is told, before anything is added, that it comes in as a document of its own', async () => {
    fs.writeFileSync(wav, silentWav());
    humor = (await rpc('code.add', { name: 'Humor' })).id;
    worry = (await rpc('code.add', { name: 'Worry' })).id;
    await page.reload();
    await api.goToImport();
    const dialog = importDialog();
    await dialog.waitFor();
    await dialog.getByRole('textbox', { name: 'File to import' }).fill(wav);
    await dialog.getByRole('button', { name: 'Check this file' }).click();
    const note = dialog.getByTestId('recording-alone-note');
    await note.waitFor();
    const said = await text(note);
    h.check(/clips-field\.wav \(audio\)/.test(said) && /no captions beside it/.test(said) && /document of its own/.test(said) && /does not copy it into the project/.test(said), `the page says what will happen: ${said.slice(0, 220)}`);
    h.equal(await dialog.getByTestId('recording-note').count(), 0, 'and does not claim words were read from captions');
    h.equal((await rpc('source.list')).filter((s) => s.name === 'clips-field').length, 0, 'checking the file added nothing yet');
    await axe('the Import Data page with a recording that has no captions');
    await dialog.getByRole('button', { name: 'Import', exact: true }).click();
    await page.getByTestId('import-finished').waitFor();
    source = (await rpc('source.list')).find((s) => s.name === 'clips-field');
    h.equal([source.source_type, source.recording.stands_alone, source.recording.kind, source.image], ['other', true, 'audio', null], 'it is a document that stands for an audio recording');
    const row = await rpc('media.get', { source_id: source.id });
    h.equal([row.found, row.stands_alone, row.file_name], [true, true, 'clips-field.wav'], 'the recording is linked and found');
    await page.getByTestId('import-finished').getByRole('button', { name: 'Done' }).click();
  });

  await S('the document shows the player, the tools for coding a stretch, and a description, and is never started', async () => {
    await page.getByTestId('media-player').waitFor();
    await waitLoaded();
    await page.getByTestId('clip-tools').waitFor();
    const s = await state();
    h.equal([s.paused, Math.abs(s.duration - 12) < 0.3], [true, true], 'the recording is paused and twelve seconds long');
    const facts = await text(page.getByTestId('source-facts'));
    h.check(/Audio recording/.test(facts) && /0 coded stretches/.test(facts), `the header says what it is: ${facts}`);
    h.check(/Audio recording clips-field\.wav/.test(await text(page.getByTestId('recording-description'))), 'its text is a description, to be replaced by a better one');
    h.check(/does not turn speech into text/.test(await text(page.getByTestId('recording-summary'))), 'and says there are no words to code');
    h.check(/The recording is at 0:00\.0/.test(await text(page.getByTestId('clip-now'))), 'it says where the recording is');
    h.check(/No stretch is marked/.test(await text(page.getByTestId('clip-draft'))), 'and that nothing is marked');
    h.check(await page.getByTestId('clip-none').isVisible(), 'and that no stretch is coded');
    h.equal(await page.getByTestId('clip-play-draft').isDisabled(), true, 'there is nothing marked to play');
    h.equal(await page.getByTestId('clip-apply').isDisabled(), true, 'and no stretch to code');
    await page.screenshot({ path: `${ctx.shots}/clips-empty.png`, fullPage: true });
    // The tools are tall: they scroll with the description, and the last control can still be reached.
    await page.getByTestId('recording-describe').scrollIntoViewIfNeeded();
    h.check(await page.getByTestId('recording-describe').isVisible(), 'the description is reached by scrolling the reader');
    await axe('a recording that stands for its document');
  });

  await S('a stretch is marked at the recording\'s time with the buttons, and the page says what is marked', async () => {
    await seek(2);
    await page.getByTestId('clip-mark-start').click();
    h.check(/The stretch starts at 0:02\. Mark where it ends\./.test(await text(page.getByTestId('clip-draft'))), 'marking the start says so and what is next');
    await page.waitForFunction(() => document.querySelector('[data-testid=clip-start-field]')?.value === '0:02', null, { timeout: 4000 }).catch(() => {});
    h.equal(await page.getByTestId('clip-start-field').inputValue(), '0:02', 'the start shows in its field');
    await seek(7.5);
    await page.getByTestId('clip-mark-end').click();
    const said = await text(page.getByTestId('clip-draft'));
    h.check(/Marked Recording, 0:02\.0 to 0:07\.5 \(0:05\.5 long\)\. Choose a code to apply it\./.test(said), `the marked stretch is told in words: ${said}`);
    await page.waitForFunction(() => document.querySelector('[data-testid=clip-start-field]')?.value === '0:02.0' && document.querySelector('[data-testid=clip-end-field]')?.value === '0:07.5', null, { timeout: 4000 }).catch(() => {});
    h.equal([await page.getByTestId('clip-start-field').inputValue(), await page.getByTestId('clip-end-field').inputValue()], ['0:02.0', '0:07.5'], 'both ends show in the fields, with tenths together');
    h.check(/Selected: Recording, 0:02\.0 to 0:07\.5 \(0:05\.5 long\) \(not coded yet\)/.test(await text(page.getByTestId('selection-line'))), 'the line under the player says what a code would go to');
    h.equal(await page.getByTestId('clip-play-draft').isDisabled(), false, 'the marked stretch can be played (the check does not)');
    h.equal((await state()).paused, true, 'marking did not start the recording');
    await axe('a stretch marked on a recording');
  });

  await S('a code is applied to the marked stretch through the code box, and it appears in the lists and on the timeline', async () => {
    await code('Humor');
    const rows = await regions();
    h.equal([rows.length, rows[0].shape, rows[0].start_ms, rows[0].end_ms, rows[0].code_id], [1, 'span', 2000, 7500, humor], 'the stretch is stored with its code and its times');
    h.equal(rows[0].text, 'Recording, 0:02.0 to 0:07.5 (0:05.5 long)', 'the engine tells it in the same words');
    await page.waitForFunction(() => document.activeElement?.getAttribute('data-testid') === 'clip-apply', null, { timeout: 4000 }).catch(() => {});
    h.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-testid')), 'clip-apply', 'the focus is back on the button that opens the code box, so coding goes on from the keyboard');
    const list = page.getByTestId('clip-list');
    await list.waitFor();
    const row = await text(list.getByTestId('clip-row').first());
    h.check(/0:02\.0 to 0:07\.5/.test(row) && /Humor/.test(row), `the list shows the times and the code: ${row}`);
    h.equal(await page.getByTestId('clip-stripe').count(), 1, 'the timeline has one stripe');
    h.check(/No stretch is marked/.test(await text(page.getByTestId('clip-draft'))), 'the marks are let go after the code is applied');
    h.check(/1 coded stretch/.test(await text(page.getByTestId('source-facts'))), 'the header counts it');
    const detail = await text(page.getByTestId('coding-detail'));
    h.check(/Recording, 0:02\.0 to 0:07\.5/.test(detail) && /A stretch of the recording/.test(detail), `Segment Details tells it: ${detail.slice(0, 160)}`);
    h.check(await page.getByTestId('detail-play-clip').isVisible(), 'and offers to play it (the check does not press it)');
    h.check((await page.getByTestId('segment-quote-clip').count()) >= 1, 'the quote is a stretch, not words');
    await axe('a recording with a coded stretch');
  });

  await S('times can be typed instead, a second code goes to a chosen stretch, and a stretch is not given a code twice', async () => {
    await page.getByTestId('clip-start-field').fill('0:05');
    await page.getByTestId('clip-start-field').press('Enter');
    await page.getByTestId('clip-end-field').fill('9');
    await page.getByTestId('clip-end-field').press('Enter');
    h.check(/Marked Recording, 0:05 to 0:09 \(0:04 long\)/.test(await text(page.getByTestId('clip-draft'))), 'a typed start and end make a stretch');
    await code('Worry');
    let rows = await regions();
    h.equal(rows.length, 2, 'the second stretch was stored');
    h.equal(rows.map((r) => [r.start_ms, r.end_ms]), [[2000, 7500], [5000, 9000]], 'it overlaps the first, and both are kept');
    // A second code on the chosen stretch.
    await page.getByTestId('clip-row').first().getByRole('button', { name: /Choose this stretch/ }).click();
    await code('Worry');
    rows = await regions();
    h.equal(rows.length, 3, 'the first stretch now has two codes');
    h.equal(rows.filter((r) => r.start_ms === 2000).map((r) => r.code_id).sort(), [humor, worry].sort(), 'Humor and Worry');
    // The same code again is not given twice.
    await code('Worry');
    h.equal((await regions()).length, 3, 'giving the same code again changed nothing');
    await page.waitForFunction(() => /already has Worry/.test(document.querySelector('[data-testid=alert]')?.textContent ?? ''), null, { timeout: 4000 }).catch(() => {});
    const said = await text(page.getByTestId('alert'));
    h.check(/This stretch already has Worry/.test(said), `and it says so: ${said}`);
    // Typing something that is not a time says so and changes nothing.
    await page.getByTestId('clip-start-field').fill('soon');
    await page.getByTestId('clip-start-field').press('Enter');
    h.check(/Type a time such as 1:05/.test(await text(page.getByTestId('clip-draft'))), 'a typed word that is not a time is told');
    h.equal(await page.getByTestId('clip-clear').isDisabled(), true, 'and nothing was marked');
    await page.getByTestId('clip-start-field').fill('0:01');
    await page.getByTestId('clip-start-field').press('Enter');
    await page.getByTestId('clip-clear').click();
    h.equal(await page.getByTestId('clip-start-field').inputValue(), '', 'clearing lets go of the marks');
    h.equal(await page.getByTestId('clip-stripe').count(), 3, 'the timeline shows every coding, overlapping ones in rows of their own');
    await page.getByTestId('clip-timeline').scrollIntoViewIfNeeded();
    await page.screenshot({ path: `${ctx.shots}/clips-coded.png` });
    const tops = await page.getByTestId('clip-stripe').evaluateAll((els) => els.map((e) => e.style.top));
    h.check(new Set(tops).size >= 2, `stretches that overlap are in different rows: ${tops.join(', ')}`);
    await axe('a recording with overlapping coded stretches');
  });

  await S('a stretch is moved with Start here and End here, the recording is not started, and an undo puts it back', async () => {
    const first = page.getByTestId('clip-row').first();
    await seek(1);
    await first.getByTestId('clip-start-here').click();
    await page.waitForFunction(() => /0:01\.0 to 0:07\.5/.test(document.querySelector('[data-testid=clip-list]')?.textContent ?? ''), null, { timeout: 5000 });
    const moved = (await regions()).filter((r) => r.end_ms === 7500);
    h.equal(moved.map((r) => r.start_ms), [1000, 1000], 'both codings of the stretch moved together');
    h.equal((await state()).paused, true, 'moving a stretch did not start the recording');
    await rpc('undo');
    await rpc('undo');
    await page.reload();
    await documentRow('clips-field').click();
    await page.getByTestId('clip-list').waitFor();
    h.equal((await regions()).filter((r) => r.end_ms === 7500).map((r) => r.start_ms), [2000, 2000], 'undo put both back');
  });

  await S('the stretches are in Retrieved Segments, a query, and an export as stretches of a recording', async () => {
    const segments = (await rpc('segments.list', {})).segments.filter((s) => s.source_id === source.id);
    h.equal([segments.length, segments.every((s) => s.kind === 'clip')], [3, true], 'three stretches are coded segments of kind clip');
    h.check(segments.every((s) => /^Recording, /.test(s.text)), 'each is told by its times');
    const result = await rpc('explore.query', { rule: { type: 'code', code_ids: [worry] }, result_unit: 'excerpts' });
    h.equal(result.items.filter((i) => i.kind === 'clip').length, 2, 'a query for Worry finds its two stretches');
    const together = await rpc('explore.query', { rule: { type: 'group', match: 'all', items: [{ type: 'code', code_ids: [humor] }, { type: 'also_coded', code_ids: [worry], within_chars: 0 }] }, result_unit: 'excerpts' });
    h.equal(together.items.filter((i) => i.kind === 'clip').length, 1, 'Humor together with Worry is the one stretch that has both');
    const file = path.join(workDir, 'clips-segments.csv');
    await rpc('export.segments', { format: 'csv', path: file, overwrite: true, source_ids: [source.id] });
    const csv = fs.readFileSync(file, 'utf8');
    h.check(/Recording from \(seconds\),Recording to \(seconds\)/.test(csv), 'the CSV has columns for the times in seconds');
    h.check(/\[Stretch of a recording\] Recording, 0:02\.0 to 0:07\.5/.test(csv) && /,2,7\.5/.test(csv), 'and each stretch is told in words and in seconds');
  });

  await S('a chosen stretch opened from a list moves the recording to where it starts, without playing it', async () => {
    const worryStretch = (await regions()).find((r) => r.code_id === worry && r.start_ms === 5000);
    await seek(0);
    await page.getByTestId('clip-row').nth(1).getByRole('button', { name: /Choose this stretch/ }).click();
    await page.waitForFunction(() => Math.abs(document.querySelector('[data-testid=media-element]').currentTime - 5) < 0.05, null, { timeout: 5000 });
    h.equal((await state()).paused, true, 'the recording is where the stretch starts and is paused');
    h.check(/Recording, 0:05\.0|Recording, 0:05 to 0:09/.test(await text(page.getByTestId('coding-detail'))), 'Segment Details shows the stretch that was chosen');
  });

  await S('a stretch can be removed, and the keys I and O mark the start and end from the player', async () => {
    const before = (await regions()).length;
    await seek(3);
    await page.getByTestId('media-element').focus();
    await page.keyboard.press('i');
    h.check(/The stretch starts at 0:03\./.test(await text(page.getByTestId('clip-draft'))), 'I marks the start where the recording is');
    await seek(4);
    await page.keyboard.press('o');
    h.check(/Marked Recording, 0:03 to 0:04/.test(await text(page.getByTestId('clip-draft'))), 'O marks the end');
    await page.keyboard.press('i');
    h.check(/is too short|must end after it starts|starts at 0:04/.test(await text(page.getByTestId('clip-draft'))), 'marking a start after the end is told, not stored');
    h.equal((await regions()).length, before, 'marking stored nothing');
    await page.getByTestId('clip-clear').click();
    // Remove the third stretch through Segment Details.
    const last = (await regions()).find((r) => r.code_id === worry && r.start_ms === 5000);
    await rpc('coding.remove', { id: -last.id });
    await page.reload();
    await documentRow('clips-field').click();
    await page.getByTestId('clip-list').waitFor();
    h.equal((await regions()).length, before - 1, 'one stretch less');
    h.equal(await page.getByTestId('clip-row').count(), 1, 'and one row in the list');
  });

  await S('the description can be rewritten, and the stretches are there when the project is opened again', async () => {
    await page.getByTestId('recording-describe').click();
    await page.getByTestId('recording-description-box').fill('Field recording of a market stall, morning.');
    await page.getByTestId('recording-description-save').click();
    await page.getByTestId('recording-description').waitFor();
    h.equal(await text(page.getByTestId('recording-description')), 'Field recording of a market stall, morning.', 'the new description is shown');
    h.equal((await rpc('source.get', { id: source.id })).text, 'Field recording of a market stall, morning.', 'and kept');
    await page.reload();
    await documentRow('clips-field').click();
    await page.getByTestId('clip-list').waitFor();
    h.equal(await page.getByTestId('clip-row').count(), 1, 'a stretch is still coded after the page is loaded again');
    await axe('a recording with a description and a coded stretch');
  });

  await S('a transcript with a recording keeps coding by words, and the tools for stretches are folded away until asked for', async () => {
    const transcript = path.join(workDir, 'clips-talk.vtt');
    fs.writeFileSync(path.join(workDir, 'clips-talk.wav'), silentWav(12));
    fs.writeFileSync(transcript, 'WEBVTT\n\n1\n00:00:00.500 --> 00:00:02.000\n<v Ann>Hello there.\n\n2\n00:00:02.500 --> 00:00:04.000\n<v Bo>Fine, thanks.\n', 'utf8');
    const done = await rpc('import.run', { path: path.join(workDir, 'clips-talk.wav') });
    const talk = (await rpc('source.list')).find((s) => s.id === done.outcome.source_ids[0]);
    h.equal([talk.source_type, talk.recording.stands_alone], ['transcript', false], 'a recording with captions is a transcript, and does not stand for its document');
    await page.reload();
    await documentRow('clips-talk').click();
    await page.getByTestId('media-player').waitFor();
    await waitLoaded();
    const details = page.getByTestId('clip-details');
    h.equal(await details.evaluate((el) => el.open), false, 'the tools for stretches are folded away');
    h.check(/Hello there\./.test(await page.getByRole('main').innerText()), 'the words of the transcript are the text');
    await details.locator('summary').click();
    await page.getByTestId('clip-tools').waitFor();
    await seek(1);
    await page.getByTestId('clip-mark-start').click();
    await seek(3);
    await page.getByTestId('clip-mark-end').click();
    await code('Humor');
    const rows = await rpc('region.list', { source_id: talk.id });
    h.equal([rows.length, rows[0].start_ms, rows[0].end_ms], [1, 1000, 3000], 'a stretch can be coded on a transcript too');
    h.equal((await rpc('coding.list', { source_id: talk.id })).length, 0, 'and no words were coded');
    await axe('a transcript with a coded stretch');
  });

  // ---- the waveform, a stretch saved as its own sound file, and the clips of a code played one after another ----
  let tone;
  let toneHumor;
  let toneWorry;
  const saveDir = path.join(workDir, 'clips-saved');

  await S('a waveform is drawn only when asked for, shows where the sound is loud, and moves the recording without playing it', async () => {
    fs.writeFileSync(path.join(workDir, 'clips-tone.wav'), toneWav());
    const done = await rpc('import.run', { path: path.join(workDir, 'clips-tone.wav') });
    tone = (await rpc('source.list')).find((x) => x.id === done.outcome.source_ids[0]);
    h.equal([tone.recording.stands_alone, tone.recording.kind], [true, 'audio'], 'a recording with no captions stands for its document');
    const mk = async (code_id, start_ms, end_ms) => rpc('region.add', { source_id: tone.id, shape: 'span', start_ms, end_ms, code_id });
    toneHumor = humor;
    toneWorry = worry;
    await mk(toneHumor, 4000, 5000);
    await mk(toneWorry, 8000, 10000);
    await page.reload();
    await documentRow('clips-tone').click();
    await page.getByTestId('media-player').waitFor();
    await waitLoaded();
    h.equal(await page.getByTestId('waveform').count(), 0, 'no waveform is drawn until it is asked for');
    h.equal(await page.getByTestId('waveform-show').isEnabled(), true, 'the button to draw it is there');
    h.equal((await state()).paused, true, 'and the recording is paused');
    await page.getByTestId('waveform-show').click();
    await page.getByTestId('waveform').waitFor({ timeout: 10000 });
    const d = await page.locator('[data-testid=waveform] path').first().getAttribute('d');
    h.check(d && d.length > 2000, `the waveform is drawn as bars (${d ? d.length : 0} characters of path)`);
    const note = await text(page.getByTestId('waveform-note'));
    h.check(/The loudest part is at 0:04\./.test(note) && /shaded parts/.test(note), `it says where the sound is loudest: ${note}`);
    h.equal(await page.getByTestId('waveform-stretch').count(), 2, 'the two coded stretches are shaded behind it');
    const slider = page.getByTestId('waveform');
    h.equal([await slider.getAttribute('role'), await slider.getAttribute('aria-valuenow')], ['slider', '0'], 'it is a slider that starts at 0');
    h.check(Math.abs(Number(await slider.getAttribute('aria-valuemax')) - 12000) < 300, 'whose end is the length of the recording');
    await page.screenshot({ path: `${ctx.shots}/clips-waveform.png`, fullPage: true });
    // A click moves the recording to that place.
    const box = await slider.boundingBox();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await page.waitForFunction(() => Math.abs(document.querySelector('[data-testid=media-element]').currentTime - 6) < 0.3, null, { timeout: 5000 });
    h.equal((await state()).paused, true, 'a click on the waveform moved the recording and did not start it');
    // The arrow keys move it by five seconds, Home goes to the start.
    await slider.focus();
    await page.keyboard.press('ArrowRight');
    await page.waitForFunction(() => Math.abs(document.querySelector('[data-testid=media-element]').currentTime - 11) < 0.4, null, { timeout: 5000 });
    await page.keyboard.press('Home');
    await page.waitForFunction(() => document.querySelector('[data-testid=media-element]').currentTime < 0.1, null, { timeout: 5000 });
    h.equal((await state()).paused, true, 'the keys moved it and did not start it');
    h.check(/0:00 of 0:1[12]/.test((await slider.getAttribute('aria-valuetext')) ?? ''), 'the slider says where it is in words');
    await axe('a recording with its waveform drawn');
  });

  await S('a stretch is saved as a WAV file that holds only that stretch, and a file that is there is never replaced', async () => {
    fs.mkdirSync(saveDir, { recursive: true });
    const box = page.getByTestId('clip-save-box');
    await box.scrollIntoViewIfNeeded();
    h.check(await box.isVisible(), 'the place to say where to save is shown beside the list of stretches');
    // With nothing typed, and no Windows window in this browser, the page asks for a place.
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.waitForFunction(() => /Type where to save the sound/.test(document.body.innerText), null, { timeout: 5000 });
    // A folder (ending in a slash) gets a name made of the document, the times, and the codes.
    await page.getByTestId('clip-save-path').fill(saveDir + path.sep);
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.getByTestId('clip-saved').waitFor({ timeout: 15000 });
    const first = path.join(saveDir, 'clips-tone 0m04s-0m05s Humor.wav');
    h.check(fs.existsSync(first), `the file is named for the document, the times, and the code: ${fs.readdirSync(saveDir).join(', ')}`);
    h.check(/Saved .*clips-tone 0m04s-0m05s Humor\.wav/.test(await text(page.getByTestId('clip-saved'))), 'the page says where it was saved');
    const a = readWav(first);
    h.equal([a.riff, a.wave, a.channels, a.rate, a.bits], ['RIFF', 'WAVE', 1, 44100, 16], 'it is a mono 16-bit WAV at 44.1 kHz');
    h.check(Math.abs(a.seconds - 1) < 0.02, `it is one second long, the length of the stretch (${a.seconds.toFixed(3)} s)`);
    h.equal(a.size, a.declared, 'its header agrees with its size');
    h.check(a.peak > 0.5, `and it holds the loud second (peak ${a.peak.toFixed(2)})`);
    // The other stretch is silent, and two seconds long.
    await page.getByTestId('clip-row').nth(1).getByTestId('clip-save').click();
    await page.waitForFunction(() => /0m08s-0m10s/.test(document.querySelector('[data-testid=clip-saved]')?.textContent ?? ''), null, { timeout: 15000 });
    const b = readWav(path.join(saveDir, 'clips-tone 0m08s-0m10s Worry.wav'));
    h.check(Math.abs(b.seconds - 2) < 0.02 && b.peak === 0, `the other stretch is two seconds of silence (${b.seconds.toFixed(3)} s, peak ${b.peak})`);
    // A file that is there is not replaced.
    const before = fs.readFileSync(first);
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.waitForFunction(() => /already exists, so nothing was saved/.test(document.body.innerText), null, { timeout: 15000 });
    h.equal(fs.readFileSync(first).equals(before), true, 'saving again named the same file, was refused, and left it as it was');
    // A name typed in full is used as it is; a folder that is not there is told.
    await page.getByTestId('clip-save-path').fill(path.join(saveDir, 'my clip.wav'));
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.waitForFunction(() => /my clip\.wav/.test(document.querySelector('[data-testid=clip-saved]')?.textContent ?? ''), null, { timeout: 15000 });
    h.check(fs.existsSync(path.join(saveDir, 'my clip.wav')), 'a typed file name is used as typed');
    await page.getByTestId('clip-save-path').fill(path.join(workDir, 'no-such-folder', 'x.wav'));
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.waitForFunction(() => /does not exist/.test(document.body.innerText), null, { timeout: 15000 });
    h.equal((await state()).paused, true, 'none of this started the recording');
    await axe('the place to save a stretch as a sound file');
  });

  await S('a recording larger than the part the window is sent at once is drawn and cut whole, not only its first part', async () => {
    // Over eleven minutes and over five megabytes: the window answers a request for a large recording with its first four megabytes only.
    fs.writeFileSync(path.join(workDir, 'clips-big.wav'), toneWav(650, 8000, 600));
    const done = await rpc('import.run', { path: path.join(workDir, 'clips-big.wav') });
    const big = (await rpc('source.list')).find((x) => x.id === done.outcome.source_ids[0]);
    h.check(big.recording.size_bytes > 4 * 1024 * 1024 + 1024, `the recording is larger than four megabytes (${big.recording.size_bytes} bytes)`);
    await rpc('region.add', { source_id: big.id, shape: 'span', start_ms: 600000, end_ms: 601000, code_id: humor });
    await page.reload();
    await documentRow('clips-big').click();
    await page.getByTestId('media-player').waitFor();
    await waitLoaded();
    await page.getByTestId('waveform-show').click();
    await page.getByTestId('waveform').waitFor({ timeout: 30000 });
    h.check(Math.abs(Number(await page.getByTestId('waveform').getAttribute('aria-valuemax')) - 650000) < 1000, 'the waveform is as long as the whole recording, 10:50');
    const note = await text(page.getByTestId('waveform-note'));
    h.check(/The loudest part is at (9:59|10:00)\./.test(note), `the loud second near the end is found: ${note}`);
    h.equal(await page.getByTestId('waveform-stretch').count(), 1, 'the stretch coded there is shaded');
    await page.getByTestId('clip-save-path').fill(saveDir + path.sep);
    await page.getByTestId('clip-row').first().getByTestId('clip-save').click();
    await page.getByTestId('clip-saved').waitFor({ timeout: 40000 });
    const file = path.join(saveDir, 'clips-big 10m00s-10m01s Humor.wav');
    h.check(fs.existsSync(file), `a stretch after the first part of the recording is saved: ${fs.readdirSync(saveDir).join(', ')}`);
    const w = readWav(file);
    h.check(Math.abs(w.seconds - 1) < 0.02 && w.peak > 0.5, `it is the loud second (${w.seconds.toFixed(3)} s, peak ${w.peak.toFixed(2)})`);
  });

  await S('the coded stretches of recordings are played one after another from Retrieved Segments, in the order of the list', async () => {
    // A short silent recording of its own, so the check plays seconds and not minutes (the browser is muted, and the sound is silence).
    fs.writeFileSync(path.join(workDir, 'clips-short.wav'), silentWav(6));
    const done = await rpc('import.run', { path: path.join(workDir, 'clips-short.wav') });
    const short = (await rpc('source.list')).find((x) => x.id === done.outcome.source_ids[0]);
    const mk = async (code_id, start_ms, end_ms) => rpc('region.add', { source_id: short.id, shape: 'span', start_ms, end_ms, code_id });
    await mk(humor, 1000, 2000);
    await mk(worry, 3000, 4500);
    await page.reload();
    const pane = page.getByRole('region', { name: /^Retrieved Segments/ });
    await pane.waitFor();
    await pane.getByRole('searchbox', { name: 'Search segments' }).fill('clips-short');
    const toggle = pane.getByTestId('clips-toggle');
    await toggle.waitFor({ timeout: 8000 });
    h.equal(await page.getByTestId('clip-playlist').count(), 0, 'the player of clips is closed until it is asked for');
    await toggle.click();
    const list = page.getByTestId('clip-playlist');
    await list.waitFor();
    h.check(/2 coded stretches of recordings from 1 document/.test(await text(list)), `it says what is in the list: ${(await text(list)).slice(0, 160)}`);
    h.equal(await text(page.getByTestId('playlist-now')), 'Nothing is playing.', 'nothing is playing');
    h.equal([await page.getByTestId('playlist-next').isDisabled(), await page.getByTestId('playlist-stop').isDisabled()], [true, true], 'next and stop have nothing to act on');
    await axe('the player of clips in Retrieved Segments');
    const now = () => text(page.getByTestId('playlist-now'));
    const el = () => page.evaluate(() => { const v = document.querySelector('[data-testid=playlist-element]'); return { time: v.currentTime, paused: v.paused }; });
    // The clips are played in the order the table is in, which is the order the engine lists them.
    // (Within one recording they are in the order of their times.)
    const listed = (await rpc('segments.list', {})).segments.filter((x) => x.source_id === short.id && x.region && x.region.shape === 'span').sort((a, b) => a.region.start_ms - b.region.start_ms);
    const tenths = (r) => r.start_ms % 1000 >= 100 || r.end_ms % 1000 >= 100;
    const clock = (ms, fine) => `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, '0')}${fine ? '.' + Math.floor((ms % 1000) / 100) : ''}`;
    const lineOf = (n) => { const r = listed[n - 1].region; return `Clip ${n} of 2: clips-short, ${clock(r.start_ms, tenths(r))} to ${clock(r.end_ms, tenths(r))}, ${listed[n - 1].code_name}.`; };
    h.equal(listed.length, 2, 'the engine lists the two stretches');
    const said = (n) => page.waitForFunction((line) => (document.querySelector('[data-testid=playlist-now]')?.textContent ?? '').includes(line), lineOf(n), { timeout: 9000 });
    // Play all: the first clip, then the second by itself.
    await page.getByTestId('playlist-play').click();
    await said(1);
    await said(2);
    const second = await el();
    const from2 = listed[1].region.start_ms / 1000;
    h.check(second.time >= from2 - 0.1 && second.time < listed[1].region.end_ms / 1000 + 0.1, `the second clip began at its start (${second.time.toFixed(2)} s)`);
    // Previous goes back, Pause holds, Resume goes on, Next moves forward, Stop ends.
    await page.getByTestId('playlist-previous').click();
    await page.waitForFunction(() => /Clip 1 of 2/.test(document.querySelector('[data-testid=playlist-now]')?.textContent ?? ''), null, { timeout: 5000 });
    await page.getByTestId('playlist-pause').click();
    h.check(/^Paused\. Clip 1 of 2/.test(await now()) && (await el()).paused, 'Pause holds the clip and says so');
    await page.getByTestId('playlist-pause').click();
    await page.waitForFunction(() => !document.querySelector('[data-testid=playlist-element]').paused, null, { timeout: 5000 });
    await page.getByTestId('playlist-next').click();
    await page.waitForFunction(() => /Clip 2 of 2/.test(document.querySelector('[data-testid=playlist-now]')?.textContent ?? ''), null, { timeout: 5000 });
    // The last clip runs to its end, and the list says it was the last.
    await page.waitForFunction(() => /That was the last clip\./.test(document.querySelector('[data-testid=playlist-now]')?.textContent ?? ''), null, { timeout: 9000 });
    h.equal((await el()).paused, true, 'after the last clip the recording is paused');
    await page.getByTestId('playlist-play').click();
    await page.waitForFunction(() => /Clip 1 of 2/.test(document.querySelector('[data-testid=playlist-now]')?.textContent ?? ''), null, { timeout: 5000 });
    await page.getByTestId('playlist-stop').click();
    h.equal([await now(), (await el()).paused], ['Nothing is playing.', true], 'Stop ends it');
    // Closing the player of clips leaves the table as it was.
    await toggle.click();
    h.equal(await page.getByTestId('clip-playlist').count(), 0, 'the player of clips closes');
    await pane.getByRole('searchbox', { name: 'Search segments' }).fill('');
  });
}
