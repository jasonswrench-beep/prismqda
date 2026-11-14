import assert from 'node:assert/strict';
import test from 'node:test';
import { tableForClipboard } from './statExport';

test('a table is put on the clipboard as tab-separated rows and as an HTML table, and what is in a cell cannot break either', () => {
  const { text, html } = tableForClipboard({
    title: 'Scores <by> group',
    head: ['Group', 'M', 'p'],
    rows: [
      ['A & B', '11.50', '< .001'],
      ['C', 'two\nlines\tand a tab', '.20'],
    ],
    note: 'M is the "mean".',
  });
  assert.equal(text, 'Group\tM\tp\nA & B\t11.50\t< .001\nC\ttwo lines and a tab\t.20', 'a spreadsheet gets one cell between each pair of tabs and one row on each line');
  assert.equal(text.split('\n').length, 3);
  assert.ok(html.startsWith('<table><caption>Scores &lt;by&gt; group</caption><thead><tr><th>Group</th><th>M</th><th>p</th></tr></thead><tbody>'), html);
  assert.ok(html.includes('<td>A &amp; B</td><td>11.50</td><td>&lt; .001</td>'));
  assert.ok(html.includes('<p>M is the &quot;mean&quot;.</p>'));
  assert.ok(!html.includes('<by>'), 'nothing in a cell is read as markup');
});

test('a table without a note puts no empty paragraph after it', () => {
  const { html } = tableForClipboard({ title: 'T', head: ['A'], rows: [['1']] });
  assert.ok(html.endsWith('</table>'), html);
});
