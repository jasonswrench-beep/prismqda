// The Team page: choosing coders, measuring their agreement one code per unit and code by code, the table of where they differ, the
// written report, a model coder called concordance, and keeping or archiving a run. The document, codes, and second coder are the
// check's own, so nothing depends on what earlier steps left in the project.

import fs from 'node:fs';
import path from 'node:path';
import { makePng } from './scenarios-image.mjs';

const RIDERS = 'Fares are too high for me.\n\nThe stop is far from my house.\n\nI love the new buses.\n\nThe driver was rude to me.\n\nThe seats are comfortable.\n\nIt always arrives late.\n';

export async function runTeam(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const result = () => page.getByTestId('agreement-result');
  /** Ticks exactly the coders named (as their check boxes say them) and unticks the rest. */
  async function choose(...names) {
    const labels = page.locator('.team-coders label');
    const count = await labels.count();
    for (let i = 0; i < count; i++) {
      const label = labels.nth(i);
      const wanted = names.includes((await label.innerText()).trim());
      if (wanted !== (await label.locator('input').isChecked())) await label.click();
    }
  }
  let me;
  // the areas the picture step draws, taken away again at the end of the final-layer step so the picture steps that follow count their own
  let teamAreas = [];
  const measure = async () => {
    await page.getByTestId('team-measure').click();
    await result().waitFor();
    await page.waitForFunction(() => !document.querySelector('[data-testid=team-measure]')?.textContent?.includes('Measuring'), null, { timeout: 15000 });
  };

  await S('the Team tab opens a page for agreement, with the coders to choose and the ways to compare them', async () => {
    fs.writeFileSync(path.join(workDir, 'team-riders.txt'), RIDERS, 'utf8');
    const done = await rpc('import.run', { path: path.join(workDir, 'team-riders.txt') });
    const sourceId = done.outcome.source_ids[0];
    const cost = (await rpc('code.add', { name: 'Team cost' })).id;
    const access = (await rpc('code.add', { name: 'Team access' })).id;
    me = (await rpc('coder.list')).find((c) => c.role === 'human');
    const other = await rpc('coder.add', { name: 'Second coder', role: 'human' });
    const units = (await rpc('unit.list', { source_id: sourceId })).filter((u) => !u.excluded);
    h.equal(units.length, 6, 'the document has six paragraphs, so six units');
    // The first coder says cost, cost, cost, cost, access, access. The second says cost, cost, access, access, access, access.
    const first = [cost, cost, cost, cost, access, access];
    const second = [cost, cost, access, access, access, access];
    for (let i = 0; i < units.length; i++) {
      await rpc('coding.apply', { source_id: sourceId, start: units[i].start, end: units[i].end, code_id: first[i], coder_id: me.id, unit_id: units[i].id });
      await rpc('coding.apply', { source_id: sourceId, start: units[i].start, end: units[i].end, code_id: second[i], coder_id: other.id, unit_id: units[i].id });
    }
    await rpc('coder.add', { name: 'Test model', role: 'model', provider: 'Nobody', model_name: 'invented', model_version: '1' });
    await page.reload();
    await page.getByRole('tab', { name: 'Team', exact: true }).click();
    h.equal(await page.getByRole('status').filter({ hasText: 'not built yet' }).count(), 0, 'the Team tab no longer says it is not built');
    await page.getByRole('button', { name: 'Team page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    const boxes = page.getByTestId('team-coder-' + me.id);
    await boxes.waitFor();
    const humans = (await rpc('coder.list')).filter((c) => c.role === 'human').length;
    h.equal(await page.locator('[data-testid^=team-coder-] input:checked').count(), humans, 'every person who codes is chosen, and the model is not');
    h.check(/Test model \(AI model\)/.test(await page.locator('.team-coders').innerText()), 'a model is named as one');
    await choose(me.name, 'Second coder');
    h.equal(await page.getByRole('radio', { name: /One code for each unit/ }).isChecked(), true, 'one code for each unit is the way to begin');
    await axe('the Team page');
  });

  await S('measuring agreement gives a sentence, the coefficients with intervals, where the coders differ, and a written report', async () => {
    await measure();
    const headline = await page.getByTestId('agreement-headline').innerText();
    h.check(/^6 units compared\. Krippendorff's alpha is -?\d\.\d{3}, which is below the target of 0\.800\.$/.test(headline), `the headline: ${headline}`);
    const overall = await page.getByTestId('agreement-overall').innerText();
    h.check(/Krippendorff's alpha/.test(overall) && /Percent agreement/.test(overall) && /0\.667/.test(overall), `the coefficients are listed, and 4 of 6 is 0.667: ${overall.replace(/\s+/g, ' ').slice(0, 200)}`);
    h.check(/\d\.\d{3} to \d\.\d{3}/.test(overall) || /-\d\.\d{3} to/.test(overall), 'an interval is given for the main coefficients');
    const confusion = await page.getByTestId('agreement-confusion').innerText();
    h.check(/Team cost/.test(confusion) && /Team access/.test(confusion), 'the table of how the two coders line up names the codes');
    h.check(/differed on 2 of 6 units, 33 percent/.test(await page.getByTestId('agreement-disagreements').innerText()), 'and says how many units they differed on');
    const text = await page.getByTestId('agreement-report-text').inputValue();
    h.check(/Second coder/.test(text) && /agree/i.test(text), 'the written report names the coders');
    h.check(/Called intercoder reliability/.test(await result().innerText()), 'two people make it intercoder reliability');
    const runs = await rpc('reliability.list');
    h.equal(runs.length >= 1, true, 'the run is kept in the project');
    await axe('the Team page with a result');
  });

  await S('each code on its own is measured per code, with the codes under the target flagged', async () => {
    await page.locator('label').filter({ hasText: /Each code on its own/ }).click();
    await measure();
    const byCode = await page.getByTestId('agreement-by-code').innerText();
    h.check(/Team cost/.test(byCode) && /Team access/.test(byCode), 'a row for each code');
    h.check(/Below the target/.test(byCode), `a code with imperfect agreement is flagged: ${byCode.replace(/\s+/g, ' ').slice(0, 220)}`);
    h.check(/code by code/.test(await page.getByTestId('agreement-headline').innerText()), 'and the sentence says it was code by code');
    h.equal(await page.getByTestId('agreement-confusion').count(), 0, 'there is no table of one code against another in this view');
  });

  await S('a model joins as a coder and the run is called concordance; fewer than two coders is refused in words', async () => {
    await choose(me.name, 'Second coder', 'Test model (AI model)');
    await page.locator('label').filter({ hasText: /One code for each unit/ }).click();
    await measure();
    h.check(/Called concordance/.test(await result().innerText()), 'with a model among the coders the run is called concordance');
    h.check(/Test model \(AI model\)/.test(await result().innerText()), 'and the model is named as one');
    // Choose only one coder.
    await choose(me.name);
    await page.getByTestId('team-measure').click();
    await page.getByText('Choose at least two coders to compare.').waitFor();
    h.check(true, 'one coder alone is refused');
  });

  await S('the Disagreements tab lists the units the coders differ on, and each is settled with a reason that is kept apart from the codings', async () => {
    const codeNamed = async (name) => (await rpc('code.list')).find((c) => c.name === name);
    const cost = await codeNamed('Team cost');
    const access = await codeNamed('Team access');
    await choose(me.name, 'Second coder');
    await page.getByTestId('team-tab-disagreements').click();
    const section = page.getByTestId('disagreements');
    await section.waitFor();
    await page.getByTestId('disagree-count').waitFor();
    h.check(/^2 of 2 units are not settled yet\.$/.test((await page.getByTestId('disagree-count').innerText()).trim()), `two units, none settled: ${await page.getByTestId('disagree-count').innerText()}`);
    const cases = section.getByTestId('disagreement');
    h.equal(await cases.count(), 2, 'the two units the coders differ on');
    h.check(/I love the new buses\./.test(await cases.nth(0).innerText()) && /The driver was rude to me\./.test(await cases.nth(1).innerText()), 'the third paragraph and then the fourth, in reading order');
    h.check(/Second coder/.test(await cases.nth(0).innerText()), 'each coder is named with what they gave');
    const codingsBefore = (await rpc('segments.list', { code_ids: [cost.id, access.id] })).total;
    await axe('the Disagreements tab');

    // Blind: nobody is named.
    await page.getByTestId('team-blind').click();
    await page.waitForFunction(() => /Coder A/.test(document.querySelector('[data-testid=disagreement]')?.textContent ?? ''), null, { timeout: 5000 });
    const blindText = await section.innerText();
    h.check(!/Second coder/.test(blindText) && /Coder B/.test(blindText), 'a blind queue calls the coders Coder A and Coder B');
    await page.getByTestId('team-blind').click();
    await page.waitForFunction(() => /Second coder/.test(document.querySelector('[data-testid=disagreement]')?.textContent ?? ''), null, { timeout: 5000 });

    // A decision without its reason is refused in words.
    const first = section.getByTestId('disagreement').nth(0);
    await first.getByTestId(`decide-code-${cost.id}`).click();
    await first.getByTestId('decision-save').click();
    await page.getByRole('alert').filter({ hasText: 'needs its reason' }).waitFor();
    h.equal((await rpc('adjudication.list')).length, 0, 'and nothing is kept');

    // With a reason it is kept, and the unit is settled.
    await first.getByTestId('decision-reason').fill('The words are about the price of a ticket.');
    await first.getByTestId('decision-save').click();
    await section.getByTestId('decision').waitFor();
    h.check(/Settled: Team cost\./.test(await section.getByTestId('decision').innerText()), 'the unit says what it was settled as');
    h.check(/1 of 2 units are not settled yet\./.test(await page.getByTestId('disagree-count').innerText()), 'one is left');
    const kept = await rpc('adjudication.list');
    h.equal([kept.length, kept[0].reason, kept[0].codes.map((k) => k.name)], [1, 'The words are about the price of a ticket.', ['Team cost']], 'the decision is in the project with its reason');
    h.equal((await rpc('segments.list', { code_ids: [cost.id, access.id] })).total, codingsBefore, "no coder's coding changed");
    h.equal(await section.getByTestId('disagreement').count(), 2, 'the one just settled stays in view, showing what was decided, beside the one that is not');

    // All of them, then change the decision to say that no code applies.
    await page.locator('label').filter({ hasText: 'All of them' }).click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=disagreement]').length === 2, null, { timeout: 5000 });
    await section.getByRole('button', { name: /^Change the decision/ }).click();
    const form = section.getByTestId('decision-form').first();
    await form.getByTestId(`decide-code-${cost.id}`).click();
    await form.getByTestId('decision-reason').fill('Neither code fits this one.');
    await form.getByTestId('decision-save').click();
    await page.waitForFunction(() => /no code applies/.test(document.querySelector('[data-testid=decision]')?.textContent ?? ''), null, { timeout: 5000 });
    const changed = await rpc('adjudication.list');
    h.equal([changed.length, changed[0].codes.length, changed[0].reason], [1, 0, 'Neither code fits this one.'], 'a second decision replaces the first, and none at all says that no code applies');
    // Undo takes it back to the first decision.
    const undone = await rpc('undo');
    h.equal(undone.action, 'adjudication.set', 'undo reverses the decision');
    h.equal((await rpc('adjudication.list'))[0].reason, 'The words are about the price of a ticket.', 'and the earlier decision is back');

    // A unit can be opened in its document.
    await section.getByRole('button', { name: /^Open the unit in/ }).first().click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor({ state: 'hidden' });
    await page.getByRole('tab', { name: 'Team', exact: true }).click();
    await page.getByRole('button', { name: 'Team page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
  });

  await S('areas of a picture the coders did not all mark are decided too, with a reason, and the decision puts them in the final layer or leaves them out', async () => {
    const png = path.join(workDir, 'team-street.png');
    fs.writeFileSync(png, makePng(240, 150));
    const done = await rpc('import.run', { path: png });
    const pictureId = done.outcome.source_ids[0];
    const cost = (await rpc('code.list')).find((c) => c.name === 'Team cost');
    const second = (await rpc('coder.list')).find((c) => c.name === 'Second coder');
    const box = (x, y) => ({ shape: 'rectangle', x, y, w: 0.3, h: 0.3 });
    // both coders mark the top left corner (a little apart); only the first coder marks the bottom right
    await rpc('region.add', { source_id: pictureId, ...box(0.05, 0.05), code_id: cost.id, coder_id: me.id });
    await rpc('region.add', { source_id: pictureId, ...box(0.08, 0.06), code_id: cost.id, coder_id: second.id });
    await rpc('region.add', { source_id: pictureId, ...box(0.6, 0.6), code_id: cost.id, coder_id: me.id, rationale: 'A sign about fares.' });
    teamAreas = (await rpc('region.list', { source_id: pictureId })).map((r) => r.id);
    h.equal(teamAreas.length, 3, 'three areas were drawn');
    let summary = await rpc('adjudication.final_summary');
    h.check(summary.areas_agreed === 1 && summary.areas_open === 1 && summary.areas_decided === 0, `one area is agreed and one is in disagreement: ${JSON.stringify(summary)}`);

    // the sources are read again, so the new picture is known to the page
    await page.reload();
    await page.getByRole('tab', { name: 'Team', exact: true }).click();
    await page.getByRole('button', { name: 'Team page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();
    await choose(me.name, 'Second coder');
    await page.getByTestId('team-tab-disagreements').click();
    const section = page.getByTestId('region-disagreements');
    await section.waitFor();
    h.check(/^Not settled yet: 1 of 1 area or stretch\.$/.test((await page.getByTestId('region-disagree-count').innerText()).trim()), `one area, not settled: ${await page.getByTestId('region-disagree-count').innerText()}`);
    const cases = section.getByTestId('region-disagreement');
    h.equal(await cases.count(), 1, 'only the area some coders did not mark is listed');
    const text = await cases.nth(0).innerText();
    h.check(/Team cost in team-street/.test(text) && new RegExp(`Marked by ${me.name}`).test(text) && /Not marked by Second coder\./.test(text), `it says who marked it and who did not: ${text.slice(0, 200)}`);
    h.equal(await cases.nth(0).getByTestId('region-thumb').count(), 1, 'the area is shown as a small picture');
    await axe('the Disagreements tab with an area in disagreement');

    // a decision needs its reason
    await cases.nth(0).getByTestId('region-accept').click();
    await page.getByText(/A decision needs its reason: say why this area belongs/).first().waitFor();
    h.equal((await rpc('adjudication.final_summary')).areas_decided, 0, 'nothing was decided without a reason');

    // blind: nobody is named
    await page.getByTestId('team-blind').click();
    await page.waitForFunction(() => /1 of 2 coders of this document marked it/.test(document.querySelector('[data-testid=region-who]')?.textContent ?? ''), null, { timeout: 5000 });
    h.check(!/Marked by/.test(await cases.nth(0).innerText()), 'blind: the coders are not named');
    await page.getByTestId('team-blind').click();
    await page.waitForFunction(() => /Not marked by Second coder/.test(document.querySelector('[data-testid=region-who]')?.textContent ?? ''), null, { timeout: 5000 });

    // accept it
    await cases.nth(0).getByTestId('region-reason').fill('The sign is about the price of a ticket.');
    await cases.nth(0).getByTestId('region-accept').click();
    await cases.nth(0).getByTestId('region-decision').waitFor();
    h.check(/Settled: it is in the final layer\./.test(await cases.nth(0).getByTestId('region-decision').innerText()), 'the decision is shown');
    summary = await rpc('adjudication.final_summary');
    h.check(summary.areas_decided === 1 && summary.areas_agreed === 1 && summary.areas_open === 0, `the area is decided: ${JSON.stringify(summary)}`);
    const finalAreas = async () => (await rpc('segments.list', { limit: 5000, layer: 'final' })).segments.filter((s) => s.region && s.source_id === pictureId);
    h.equal((await finalAreas()).length, 2, 'the agreed area and the accepted area are in the final layer');
    h.check((await finalAreas()).every((s) => s.layer === 'final'), 'and say so');
    h.equal((await rpc('segments.list', { limit: 5000, layer: 'coder' })).segments.filter((s) => s.region && s.source_id === pictureId).length, 3, "every coder's own areas are still there, none changed");
    await axe('the Disagreements tab with the area decided');

    // change it: leave it out
    await cases.nth(0).getByRole('button', { name: /^Change the decision for the Team cost/ }).click();
    await cases.nth(0).getByTestId('region-reason').fill('The sign is not about the service.');
    await cases.nth(0).getByTestId('region-leave-out').click();
    await page.waitForFunction(() => /left out of the final layer/.test(document.querySelector('[data-testid=region-decision]')?.textContent ?? ''), null, { timeout: 5000 });
    h.equal((await finalAreas()).length, 1, 'left out, only the agreed area is in the final layer');

    // undo takes it back to the first decision
    const undone = await rpc('undo');
    h.equal(undone.action, 'adjudication.region.set', 'undo reverses the decision');
    h.equal((await finalAreas()).length, 2, 'and the area is in the final layer again');
    const queue = await rpc('adjudication.region_queue');
    h.check(queue.length === 1 && queue[0].decision?.accepted === true && queue[0].decision.reason === 'The sign is about the price of a ticket.', 'with the earlier reason');
  });

  await S('the final layer is the decisions and what the coders agreed on; the segment export and the Working Set can use it', async () => {
    const summary = await rpc('adjudication.final_summary');
    h.check(summary.decided === 1 && summary.open === 1, `one unit is decided and one is still in disagreement: ${JSON.stringify(summary)}`);
    const all = await rpc('segments.list', { limit: 5000 });
    const fin = await rpc('segments.list', { limit: 5000, layer: 'final' });
    h.check(fin.segments.length > 0 && fin.segments.every((s) => s.layer === 'final'), 'every segment of the final layer says so');
    h.check(fin.total < all.total, `it holds fewer codings than the coders' own (${fin.total} against ${all.total})`);
    h.check(fin.segments.some((s) => /I love the new buses/.test(s.text) && s.rationale === 'The words are about the price of a ticket.'), 'the decision is in it, with its reason');
    h.check(!fin.segments.some((s) => /The driver was rude/.test(s.text)), 'a unit still in disagreement is not');

    // the segment export takes it
    const file = path.join(workDir, 'final-layer-segments');
    await rpc('export.segments', { format: 'csv', path: file, layer: 'final', overwrite: true });
    const csv = fs.readFileSync(file + '.csv', 'utf8');
    h.check(/I love the new buses/.test(csv) && !/The driver was rude/.test(csv), 'the export holds the decision and leaves the open unit out');

    // the Working Set dialog offers it, says how it stands, and the bar says it is on
    await page.getByRole('tab', { name: 'View', exact: true }).click();
    await page.locator('[data-command="working-set"]').click();
    await page.getByTestId('ws-layer').waitFor();
    await page.getByText(/still in disagreement/).waitFor();
    h.check(/1 unit decided/.test(await page.getByTestId('ws-final-summary').innerText()), `the dialog says how many units are decided: ${await page.getByTestId('ws-final-summary').innerText()}`);
    h.check(/Areas of pictures and stretches of recordings: 1 decided, 1 agreed, 0 still in disagreement/.test(await page.getByTestId('ws-final-summary').innerText()), `and how many areas: ${await page.getByTestId('ws-final-summary').innerText()}`);
    await axe('the working set dialog with the final layer');
    await page.getByTestId('ws-final-layer').click();
    await page.getByTestId('ws-apply').click();
    await page.getByTestId('working-set-bar').waitFor();
    h.check(/the final layer/.test(await page.getByTestId('working-set-parts').innerText()), 'the bar says the final layer is on');
    await page.getByTestId('working-set-reset').click();
    await page.getByTestId('working-set-bar').waitFor({ state: 'detached' });
    await page.getByRole('tab', { name: 'Team', exact: true }).click();
    await page.getByRole('button', { name: 'Team page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Team', exact: true }).waitFor();

    // taking the areas away takes their decision with them
    for (const id of teamAreas) await rpc('coding.remove', { id: -id });
    const after = await rpc('adjudication.final_summary');
    h.check(after.areas_decided === 0 && after.areas_agreed === 0 && after.areas_open === 0, `no area is left to decide: ${JSON.stringify(after)}`);
  });

  await S('runs are listed, opened again, and archived after asking', async () => {
    await page.getByTestId('team-tab-agreement').click();
    const runs = await rpc('reliability.list');
    h.equal(runs.length >= 3, true, `three runs so far: ${runs.length}`);
    const table = page.getByTestId('agreement-runs');
    await table.waitFor();
    const first = runs[runs.length - 1].id;
    await table.getByRole('button', { name: `Open run ${first}`, exact: true }).click();
    await page.getByRole('heading', { level: 3, name: `Result of run ${first}` }).waitFor();
    await table.getByRole('button', { name: `Archive run ${first}`, exact: true }).click();
    h.equal((await rpc('reliability.list')).find((r) => r.id === first).archived, false, 'asking does not archive it');
    await table.getByRole('button', { name: 'Keep it' }).click();
    await table.getByRole('button', { name: `Archive run ${first}`, exact: true }).click();
    await table.getByRole('button', { name: `Archive run ${first} now`, exact: true }).click();
    await page.waitForFunction(() => /Archived/.test(document.querySelector('[data-testid=agreement-runs]')?.textContent ?? ''), null, { timeout: 5000 });
    h.equal((await rpc('reliability.list')).find((r) => r.id === first).archived, true, 'archiving takes the run out of use');
    await axe('the Team page with the runs listed');
  });
}
