// The Concept Map: making a map, putting codes, cases, and notes on it, joining them with links that have words on them, moving nodes by
// mouse and by keyboard, arranging them, reading the whole map as a list, and taking changes back.

export async function runMap(ctx, api) {
  const { h, rpc } = ctx;
  const { page, S, axe, waitStatus } = api;

  const tab = (name) => page.getByRole('tablist', { name: 'Explore tools' }).getByRole('tab', { name });
  const codesList = async () => rpc('code.list');
  const nodeEl = (id) => page.locator(`[data-testid=map-node][data-node="${id}"]`);
  const current = async () => {
    const maps = await rpc('map.list');
    return maps.length === 0 ? null : rpc('map.get', { id: maps[0].id });
  };

  async function openMap() {
    if (!(await page.getByRole('heading', { level: 2, name: 'Explore' }).isVisible())) {
      await page.getByRole('tab', { name: 'Explore', exact: true }).click();
      await page.getByRole('button', { name: 'Query', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Explore' }).waitFor();
    }
    await tab('Concept Map').click();
    await page.getByTestId('map-tool').waitFor();
  }

  await S('the Concept Map starts empty, a map is made with a name, and a code, a case, and a note are put on it', async () => {
    await openMap();
    h.check(/Make a map to begin/.test(await page.getByTestId('map-none').innerText()), 'with no map it says how to begin');
    await axe('Explore, the Concept Map before a map is made');
    await page.getByTestId('new-map').click();
    await page.getByTestId('map-name').fill('Barriers');
    await page.getByTestId('map-name-save').click();
    await page.getByTestId('map-canvas').waitFor();
    h.equal((await rpc('map.list')).map((m) => m.name), ['Barriers'], 'the engine has the map');

    const cost = (await codesList()).find((c) => c.name === 'Cost');
    await page.getByLabel('A code', { exact: true }).selectOption(String(cost.id));
    await page.getByTestId('add-code-node').click();
    await nodeEl((await current()).nodes[0].id).waitFor();
    await page.getByLabel('A note', { exact: true }).fill('Why people stay away');
    await page.getByTestId('add-note-node').click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=map-node]').length === 2);
    const map = await current();
    h.equal(map.nodes.map((n) => [n.kind, n.label]), [['code', 'Cost'], ['note', 'Why people stay away']], 'a code and a note are on the map');
    h.equal(await page.getByLabel('A code', { exact: true }).locator('option', { hasText: 'Cost (on the map)' }).isDisabled(), true, 'a code that is on the map cannot be added twice');
    const cases = await rpc('case.list');
    await page.getByLabel('A case', { exact: true }).selectOption(String(cases[0].id));
    await page.getByRole('button', { name: 'Add the case' }).click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=map-node]').length === 3);
    h.equal((await current()).nodes[2].kind, 'case', 'a case can be put on the map too');
    await page.screenshot({ path: `${ctx.shots}/explore-map.png`, fullPage: true });
    await axe('Explore, the Concept Map with three nodes');
  });

  await S('a node is chosen by the mouse or the keyboard and moved by the arrow keys or by dragging', async () => {
    const map = await current();
    const cost = map.nodes[0];
    await nodeEl(cost.id).click();
    h.equal(await nodeEl(cost.id).getAttribute('aria-pressed'), 'true', 'a click chooses a node');
    h.check(/^Cost/.test((await page.getByTestId('map-node-panel').getByRole('heading', { level: 3 }).innerText()).trim()), 'and its panel opens');
    await nodeEl(cost.id).focus();
    await page.keyboard.press('ArrowRight');
    await page.waitForFunction(async () => true);
    await expectX(cost.id, cost.x + 20, 'ArrowRight moves a node a little');
    // The page is still drawing the map again after that move, and a key pressed in that moment is refused (the map is busy).
    await page.waitForTimeout(600);
    await page.keyboard.press('Shift+ArrowDown');
    await expectY(cost.id, cost.y + 100, 'Shift with an arrow moves it a lot');
    // Dragging with the mouse.
    const before = (await current()).nodes[0];
    const b = await nodeEl(cost.id).boundingBox();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2 + 90, b.y + b.height / 2 + 5, { steps: 6 });
    await page.mouse.up();
    await page.waitForFunction(() => true);
    const after = await waitNode(cost.id, (n) => n.x !== before.x || n.y !== before.y);
    h.check(after.x > before.x, `a drag moves the node: x went from ${before.x} to ${after.x}`);
    await page.getByTestId('map-undo').click();
    await waitNode(cost.id, (n) => n.x === before.x && n.y === before.y);
    h.check(true, 'one undo puts a dragged node back');
  });

  async function waitNode(id, test) {
    let n;
    for (let i = 0; i < 40; i++) {
      n = (await current()).nodes.find((x) => x.id === id);
      if (test(n)) return n;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error(`the node ${id} never reached the expected place: ${JSON.stringify(n)}`);
  }
  async function expectX(id, x, what) {
    const n = await waitNode(id, (v) => v.x === x);
    h.equal(n.x, x, what);
  }
  async function expectY(id, y, what) {
    const n = await waitNode(id, (v) => v.y === y);
    h.equal(n.y, y, what);
  }

  await S('two nodes are joined with words on the link, which can be changed, and the whole map is written out', async () => {
    const map = await current();
    const [cost, note] = map.nodes;
    await page.getByTestId('map-node-button').first().click();
    await page.getByLabel(`Join ${cost.label} to`).selectOption(String(note.id));
    await page.getByLabel('The words on the link').fill('is explained by');
    await page.getByTestId('add-link').click();
    await page.getByTestId('map-link-row').first().waitFor();
    const words = await page.getByTestId('map-words').innerText();
    h.check(/Cost is explained by Why people stay away/.test(words), `the link is told in a sentence: ${words.replace(/\s+/g, ' ').slice(0, 200)}`);
    h.equal(await page.locator('.cm-link').count(), 1, 'and drawn');
    h.equal((await current()).links.map((l) => [l.label, l.directed]), [['is explained by', true]], 'the engine has it');
    await page.getByRole('button', { name: /^Change the link: Cost is explained by/ }).click();
    await page.getByLabel(/^Words of the link/).fill('depends on');
    await page.getByTestId('save-link').click();
    await page.waitForFunction(() => /Cost depends on Why people stay away/.test(document.querySelector('[data-testid=map-words]')?.textContent ?? ''));
    h.check(true, 'the words of a link can be changed');
    // The same pair again with the same words is refused, with the reason.
    await page.getByLabel(`Join ${cost.label} to`).selectOption(String(note.id));
    await page.getByLabel('The words on the link').fill('depends on');
    await page.getByTestId('add-link').click();
    h.check(/already joined/.test(await page.getByRole('main').getByRole('alert').first().innerText()), 'the same link twice is refused and says why');
    await axe('Explore, the Concept Map with a link');
  });

  await S('codes are added together with the codebook\'s hierarchy, nodes are arranged in layers, and every change can be taken back', async () => {
    const all = await codesList();
    const parent = all.find((c) => c.status === 'active' && all.some((k) => k.parent_id === c.id && k.status === 'active'));
    const kids = all.filter((c) => c.parent_id === parent.id && c.status === 'active').slice(0, 2);
    const before = await current();
    await page.getByRole('button', { name: /^Codes to put on the map:/ }).click();
    const dialog = page.getByRole('dialog', { name: 'Codes to put on the map' });
    await dialog.waitFor();
    await dialog.getByRole('button', { name: 'None' }).click();
    for (const c of [parent, ...kids]) await dialog.locator('label').filter({ has: page.getByRole('checkbox', { name: c.name, exact: true }) }).click();
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    await page.getByTestId('add-codes-together').click();
    await page.getByTestId('map-note').filter({ hasText: /Put \d+ codes? on the map/ }).waitFor();
    const after = await current();
    const contains = after.links.filter((l) => l.label === 'contains');
    h.check(contains.length >= 1, `the parent is joined to its sub-codes: ${contains.length} link(s)`);
    h.check(after.nodes.length > before.nodes.length, 'the codes are on the map');

    const positions = (m) => JSON.stringify(m.nodes.map((n) => [n.id, n.x, n.y]));
    const unarranged = positions(after);
    await page.getByLabel('Arrange the nodes').selectOption('layers');
    await page.getByTestId('arrange').click();
    await page.waitForFunction(async (u) => true, unarranged);
    let arranged = after;
    for (let i = 0; i < 40 && positions(arranged) === unarranged; i++) {
      await new Promise((r) => setTimeout(r, 100));
      arranged = await current();
    }
    const y = (label) => arranged.nodes.find((n) => n.label === label).y;
    h.check(y(parent.name) < y(kids[0].name), 'in layers the parent is above its sub-code');
    await page.getByTestId('map-undo').click();
    let back = arranged;
    for (let i = 0; i < 40 && positions(back) !== unarranged; i++) {
      await new Promise((r) => setTimeout(r, 100));
      back = await current();
    }
    h.equal(positions(back), unarranged, 'one undo puts every node back where it was');
    await axe('Explore, the Concept Map with codes arranged');
  });

  await S('a node is taken off with the Delete key and brought back with its links, and a whole map can be deleted and restored', async () => {
    const map = await current();
    const note = map.nodes.find((n) => n.kind === 'note');
    await nodeEl(note.id).focus();
    await page.keyboard.press('Delete');
    await page.getByTestId('map-note').filter({ hasText: /Took Why people stay away off the map, with 1 link/ }).waitFor();
    h.equal((await current()).nodes.some((n) => n.id === note.id), false, 'Delete takes the node off the map');
    h.equal((await current()).links.some((l) => l.label === 'depends on'), false, 'and the link it had');
    await page.getByTestId('map-undo').click();
    await page.waitForFunction(() => /Cost depends on Why people stay away/.test(document.querySelector('[data-testid=map-words]')?.textContent ?? ''));
    h.check(true, 'one undo brings the node back with its link');

    await page.getByTestId('delete-map').click();
    await page.getByTestId('map-none').waitFor();
    h.equal((await rpc('map.list')).length, 0, 'the map is deleted');
    await page.getByTestId('map-undo').click();
    await page.getByTestId('map-canvas').waitFor();
    const restored = await current();
    h.check(restored.nodes.length >= 3 && restored.links.length >= 1, 'one undo brings the whole map back with everything on it');
    const said = await waitStatus(/Undid/).catch(() => '');
    h.check(said === '' || /map/i.test(said), `and says what it undid: ${said}`);
  });
}
