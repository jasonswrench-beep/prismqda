// The words of the window. The window is opened in the test language (?lang=en-XA), in which every message that goes through the
// language catalog is bent into accented letters and framed, so a word that is still plain English was written into the program
// without going through the catalog. Each screen is visited and every visible word, and every label read aloud, is checked.
// What the project itself holds (the names of codes and documents, the text being coded, memos) is the person's own and stays as it is.

import path from 'node:path';
import { runTranslations } from './scenarios-translations.mjs';

/** Words that are the same in every language: names of the program and of file kinds, and the like. */
const SAME_EVERYWHERE = new Set(
  [
    'PrismQDA', 'Inter', 'PDF', 'DOCX', 'TXT', 'CSV', 'TSV', 'XLSX', 'XLSM', 'SPSS', 'RTF', 'HTML', 'MHTML', 'RSS', 'Atom', 'XML', 'EML', 'MBOX', 'PNG', 'JPEG', 'JPG', 'GIF', 'WebP', 'BMP',
    'MP3', 'MP4', 'WAV', 'VTT', 'SRT', 'JSON', 'RIS', 'BibTeX', 'CSL', 'QDC', 'OpenDocument', 'Word', 'Excel', 'Windows', 'WebView', 'Microsoft', 'ODT', 'MD', 'Markdown', 'ODS',
    'Ctrl', 'Alt', 'Shift', 'Enter', 'Escape', 'Tab', 'Esc', 'Delete', 'Backspace', 'Home', 'End', 'Page', 'Up', 'Down', 'Left', 'Right', 'Space', 'Del', 'F1', 'F6',
    'px', 'ms', 'rem', 'em', 'vs', 'id', 'ID', 'AI', 'URL', 'OK', 'pqda', 'docx', 'txt', 'csv', 'xlsx', 'png', 'jpg', 'md', 'html', 'json',
    // written by the browser for the language in use
    'jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec', 'am', 'pm',
    // the few words the component library supplies itself; it is told the language of the window (see LocaleBoundary)
    'dismiss', 'suggestions', 'options', 'available', 'option', 'clear', 'search', 'collapse', 'expand',
    // the name of the example folder in a path shown as an example
    'reports',
  ].map((w) => w.toLowerCase()),
);

export async function runLanguage(ctx, api) {
  const { h, rpc, base, workDir } = ctx;
  const { page, S } = api;

  /** The words the person's own project holds. */
  let vocabulary = new Set();
  const wordsOf = (value) => (JSON.stringify(value) ?? '').match(/[A-Za-z][A-Za-z']+/g) ?? [];

  async function learnProject() {
    const words = new Set();
    const add = (v) => wordsOf(v).forEach((w) => words.add(w.toLowerCase()));
    add(await rpc('sample.list').catch(() => []));
    const sources = await rpc('source.list');
    add(sources);
    for (const s of sources) add(await rpc('source.get', { id: s.id }).catch(() => null));
    add(await rpc('code.list'));
    add(await rpc('case.list'));
    add(await rpc('case.variables').catch(() => []));
    add(await rpc('coder.list').catch(() => []));
    add(await rpc('memo.list', {}).catch(() => []));
    add(await rpc('project.info'));
    add(await rpc('segments.list', {}).catch(() => []));
    add(await rpc('query.list').catch(() => []));
    add(await rpc('proposal.batches').catch(() => []));
    add(await rpc('proposal.list', { limit: 500, offset: 0 }).catch(() => []));
    add(await rpc('ai.runs').catch(() => []));
    add(await rpc('events.list', { limit: 500 }).catch(() => []));
    const maps = await rpc('map.list').catch(() => []);
    add(maps);
    for (const m of maps) add(await rpc('map.get', { id: m.id }).catch(() => null));
    vocabulary = words;
  }

  /** Every visible word and every label of the page, as { text, kind, where }. */
  const collect = () =>
    page.evaluate(() => {
      const out = [];
      const describe = (el) => {
        const bits = [];
        for (let e = el; e && e !== document.body && bits.length < 3; e = e.parentElement) {
          const id = e.getAttribute('data-testid') || e.id || e.getAttribute('aria-label') || e.className;
          bits.push(`${e.tagName.toLowerCase()}${id ? `[${String(id).slice(0, 40)}]` : ''}`);
        }
        return bits.reverse().join(' > ');
      };
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) {
        const node = walker.currentNode;
        const text = node.nodeValue.replace(/\s+/g, ' ').trim();
        const el = node.parentElement;
        if (!text || !el || ['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEXTAREA'].includes(el.tagName)) continue;
        const style = getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden') continue;
        // The names of languages in the list of languages are written by the browser, each in its own language.
        if (el.closest('option[lang]')) continue;
        // The text being coded and the quotes from it are the person's own.
        if (el.closest('.seg, [data-testid="surface"], [data-user-text]')) continue;
        out.push({ text, kind: 'text', where: describe(el) });
      }
      for (const el of document.querySelectorAll('[aria-label],[title],[placeholder],[alt],[aria-description],[aria-roledescription]')) {
        for (const a of ['aria-label', 'title', 'placeholder', 'alt', 'aria-description', 'aria-roledescription']) {
          const v = el.getAttribute(a);
          if (v && v.trim() && !el.closest('.seg, [data-testid="surface"], [data-user-text]')) out.push({ text: v.trim(), kind: a, where: describe(el) });
        }
      }
      return out;
    });

  const found = new Map(); // text -> { screens: Set, kind }

  async function scan(screen) {
    await page.waitForTimeout(250);
    const items = await collect();
    let bent = 0;
    for (const it of items) {
      const words = (it.text.match(/[A-Za-z][A-Za-z']*/g) ?? []).map((w) => w.replace(/'+$/, '').replace(/'s$/, ''));
      // Initials ("AL") are made from the person's names; dates and times are written by the browser for the language in use.
      // A quote that was cut short ("especiall...") ends in part of one of the person's own words.
      const part = (w) => (w.length >= 3 || it.text.includes(`${w}...`)) && [...vocabulary].some((v) => v.startsWith(w.toLowerCase()));
      const plain = words.filter((w) => w.length >= 2 && !/^[A-Z]{2,3}$/.test(w) && !vocabulary.has(w.toLowerCase()) && !SAME_EVERYWHERE.has(w.toLowerCase()) && !part(w));
      if (it.text.includes('⟦')) bent++;
      if (plain.length === 0) continue;
      const key = `${it.kind}: ${it.text.slice(0, 160)}`;
      const entry = found.get(key) ?? { screens: new Set(), where: it.where, words: plain.slice(0, 4) };
      entry.screens.add(screen);
      found.set(key, entry);
    }
    return { total: items.length, bent };
  }

  const go = (code) => page.evaluate(code);
  const hook = (body) => `(async () => { const { appStore, actions } = window.__prismqda; ${body} })()`;

  await S('the window opens in the test language, in which the catalog bends every word it holds', async () => {
    let count = 0;
    try {
      count = (await rpc('source.list')).length;
    } catch {
      count = 0;
    }
    // On its own (without the steps before it) the project open is a blank one; a project with documents and codings is wanted.
    if (count === 0) await rpc('sample.create', { id: 'urban-mobility', path: path.join(workDir, 'Language check.pqda') });
    await learnProject();
    h.check(vocabulary.size > 50, `the words of the project are known (${vocabulary.size})`);
    await page.goto(`${base}/?lang=en-XA`);
    await page.waitForFunction(() => !!window.__prismqda && document.querySelector('#root')?.children.length > 0);
    const r = await scan('the first screen');
    h.check(r.bent > 5, `the first screen is in the test language (${r.bent} framed words of ${r.total})`);
    h.equal(await page.evaluate(() => document.documentElement.lang), 'en', 'the page says which language it is in');
    h.equal(await page.evaluate(() => document.documentElement.dir), 'ltr', 'and which way it reads');
  });

  await S('every screen of the coding workspace has all its words in the catalog', async () => {
    await go(hook(`actions.goTo('code');`));
    // each ribbon tab, one at a time, since the commands on show are different
    for (const tab of ['home', 'import', 'code', 'memos', 'explore', 'mixed', 'visual', 'synthesize', 'text', 'stats', 'report', 'team', 'ai', 'view']) {
      await go(hook(`actions.setRibbonTab(${JSON.stringify(tab)});`));
      await scan(`the ribbon, ${tab} tab`);
    }
    await go(hook(`actions.setRibbonTab('code');`));
    const sources = await rpc('source.list');
    const codings = await rpc('segments.list', {}).catch(() => []);
    await go(hook(`await actions.openAt(${sources[0].id}, 0, 1);`));
    await scan('a document open in the reader');
    if (codings[0]) {
      await go(hook(`actions.selectCoding(${codings[0].coding_id ?? codings[0].id}, { openDetails: true });`));
      await scan('a coding chosen, with its details');
    }
    await go(hook(`actions.showPassages();`));
    await scan('the retrieved segments table');
    h.check(true, 'visited');
  });

  const workspaces = ['home', 'data', 'import', 'codebook', 'autocode', 'ai', 'synthesize', 'report', 'settings'];
  await S('every other page of the program has all its words in the catalog', async () => {
    for (const w of workspaces) {
      await go(hook(`actions.goTo(${JSON.stringify(w)});`));
      await scan(`the ${w} page`);
    }
    await go(hook(`appStore.set({ homeView: 'activity' }); actions.goTo('home');`));
    await scan('the activity history');
    await go(hook(`actions.goAutoCode('queue');`));
    await scan('the Auto Code review queue');
    await go(hook(`actions.goAi('log');`));
    await scan('the AI use log');
    for (const tool of ['query', 'matrix', 'crosstab', 'relations', 'summary', 'visualize', 'map']) {
      await go(hook(`actions.goExplore(${JSON.stringify(tool)});`));
      await scan(`Explore, ${tool}`);
    }
    h.check(true, 'visited');
  });

  await S('every dialog has all its words in the catalog', async () => {
    await go(hook(`actions.goTo('code');`));
    const dialogs = [
      { kind: 'help' },
      { kind: 'palette', mode: 'commands' },
      { kind: 'palette', mode: 'search' },
      { kind: 'export', what: 'codebook' },
      { kind: 'export', what: 'segments' },
      { kind: 'new-code', parentId: null },
      { kind: 'new-case' },
      { kind: 'snapshot' },
      { kind: 'codebook-compare' },
      { kind: 'pick-code' },
      { kind: 'import-folder' },
      { kind: 'new-project' },
      { kind: 'open-project' },
      { kind: 'sample-projects' },
      { kind: 'add-language' },
      { kind: 'translator-file' },
    ];
    for (const d of dialogs) {
      await go(hook(`appStore.set({ dialog: ${JSON.stringify(d)} });`));
      await scan(`the ${d.kind} dialog`);
      await go(hook(`appStore.set({ dialog: null });`));
    }
    h.check(true, 'visited');
  });

  await S('the window goes back to English', async () => {
    // the steps after this one look for controls by their English names
    await page.goto(base);
    await page.waitForFunction(() => document.querySelector('#root')?.children.length > 0);
    h.equal(await page.evaluate(() => document.documentElement.lang), 'en', 'the page is in English again');
  });

  await runTranslations(ctx, api);

  await S('no word of the window was written into the program without going through the catalog', async () => {
    const lines = [...found.entries()].map(([k, v]) => `${k}   <${[...v.screens].slice(0, 2).join('; ')}> (${v.words.join(', ')})`);
    if (lines.length) console.log(`       plain English found:\n         ${lines.join('\n         ')}`);
    h.equal(lines.length, 0, `${lines.length} piece(s) of text are not in the catalog`);
  });
}
