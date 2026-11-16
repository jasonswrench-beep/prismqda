// Statistics: the variables of cases, the test chooser that offers only valid tests, and the output notebook with its tables,
// results sentence, reading, and the conditions each test depends on. The cases and their variables are the check's own (twelve
// invented respondents imported from a file, with variable names no earlier step uses), and every number below was worked out
// by hand.
//
//   Stat group:  A A A A  B B B B  C C C C
//   Stat score:  10 12 11 13  15 17 16 18  20 22 21 23
//   Stat age:    21 25 33 41  29 38 44 52  31 36 47 55
//   Stat site:   N S N S  N S N S  N S N S
//   Stat hours:  5 8 6 9  7 12 10 14  11 15 13 18
//   Stat rate:   1 1 1 2  2 2 3 3  3 4 4 4          (ordered 1 to 4)
//   Stat grade:  1 1 2 1  2 2 3 3  4 3 4 4          (ordered 1 to 4)
//   Stat later:  N N N N  N N N N  N N S S          (what Stat site said, asked again)

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { unzipEntry } from './scenarios-export.mjs';
import { useTests } from './scenarios-scope.mjs';

const ROWS = [
  ['R1', 'A', 10, 21], ['R2', 'A', 12, 25], ['R3', 'A', 11, 33], ['R4', 'A', 13, 41],
  ['R5', 'B', 15, 29], ['R6', 'B', 17, 38], ['R7', 'B', 16, 44], ['R8', 'B', 18, 52],
  ['R9', 'C', 20, 31], ['R10', 'C', 22, 36], ['R11', 'C', 21, 47], ['R12', 'C', 23, 55],
];

const HOURS = [5, 8, 6, 9, 7, 12, 10, 14, 11, 15, 13, 18];
const RATE = [1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4];
const GRADE = [1, 1, 2, 1, 2, 2, 3, 3, 4, 3, 4, 4];
const LATER = ['N', 'N', 'N', 'N', 'N', 'N', 'N', 'N', 'N', 'N', 'S', 'S'];
const TEAM = ['T1', 'T2', 'T3', 'T4', 'T5', 'T6', 'T1', 'T2', 'T3', 'T4', 'T5', 'T6'];

export async function runStats(ctx, api) {
  const { h, rpc, workDir } = ctx;
  const { page, S, axe } = api;

  const open = async () => {
    if (!(await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).isVisible())) {
      await page.getByRole('tab', { name: 'Statistics', exact: true }).click();
      await page.getByRole('button', { name: 'Statistics page', exact: true }).click();
      await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).waitFor();
    }
  };
  const goal = (id) => page.getByTestId(`stats-goal-${id}`).check();
  const pick = async (role, label) => page.getByTestId(`stats-role-${role}`).selectOption({ label });
  const run = async () => {
    const before = await page.getByTestId('stats-page').count();
    await page.getByTestId('stats-run').click();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length > n, before, { timeout: 10000 });
  };
  const lastPage = () => page.getByTestId('stats-page').last();
  // PRISMQDA_CHART_SHOTS=<folder> saves a picture of each chart, so they can be looked at as well as measured.
  const shoot = async (locator, name) => {
    const dir = process.env.PRISMQDA_CHART_SHOTS;
    if (!dir) return;
    fs.mkdirSync(dir, { recursive: true });
    await locator.screenshot({ path: path.join(dir, `${name}.png`) });
    if (process.env.PRISMQDA_CHART_DARK) {
      await page.emulateMedia({ colorScheme: 'dark' });
      await locator.screenshot({ path: path.join(dir, `${name}-dark.png`) });
      await page.emulateMedia({ colorScheme: 'light' });
    }
  };
  const text = async (locator) => (await locator.innerText()).replace(/\s+/g, ' ').trim();
  // PRISMQDA_CHART_SHOTS=<folder> also keeps the files the export steps made, so they can be opened and looked at.
  const keep = (...files) => {
    const dir = process.env.PRISMQDA_CHART_SHOTS;
    if (!dir) return;
    fs.mkdirSync(dir, { recursive: true });
    for (const f of files) fs.copyFileSync(f, path.join(dir, path.basename(f)));
  };

  // The Statistics page describes unless the person turns the tests on; the steps below use the tests.
  await S('the tests are turned on in Settings for the steps that follow', async () => {
    await useTests(api);
  });

  await S('Statistics opens from its tab, lists the variables of cases with their levels, and says how many cases there are', async () => {
    let csv = 'id,Stat group,Stat score,Stat age,Stat site,Stat hours,Stat rate,Stat grade,Stat later,Stat team,answer\n';
    ROWS.forEach(([id, group, score, age], i) => {
      csv += `${id},${group},${score},${age},${i % 2 === 0 ? 'N' : 'S'},${HOURS[i]},${RATE[i]},${GRADE[i]},${LATER[i]},${TEAM[i]},I felt welcome here number ${id}.\n`;
    });
    const file = path.join(workDir, 'stat-cases.csv');
    fs.writeFileSync(file, csv, 'utf8');
    const mapping = { id_column: 'id', text_columns: ['answer'], attribute_columns: [
      { name: 'Stat group', level: 'nominal' }, { name: 'Stat score', level: 'ratio' }, { name: 'Stat age', level: 'ratio' }, { name: 'Stat site', level: 'nominal' },
      { name: 'Stat hours', level: 'ratio' }, { name: 'Stat rate', level: 'ordinal' }, { name: 'Stat grade', level: 'ordinal' }, { name: 'Stat later', level: 'nominal' }, { name: 'Stat team', level: 'nominal' }] };
    await rpc('import.run', { path: file, mapping });
    await page.reload();
    await open();
    await page.getByTestId('stats-case-count').waitFor();
    const count = await text(page.getByTestId('stats-case-count'));
    h.check(/\d+ cases/.test(count), `how many cases there are is said: ${count}`);
    const score = page.getByTestId('stats-var').filter({ hasText: 'Stat score' });
    h.check(/scale, 12 values/.test(await text(score)), `a scale variable says its level and its values: ${await text(score)}`);
    const group = page.getByTestId('stats-var').filter({ hasText: 'Stat group' });
    h.check(/nominal, 12 values/.test(await text(group)), 'a nominal variable says so');
    await page.getByLabel('Find a variable').fill('stat sc');
    h.equal(await page.getByTestId('stats-var').count(), 1, 'the variable list can be searched');
    await page.getByLabel('Find a variable').fill('');
    h.check(await page.getByTestId('stats-empty').isVisible(), 'the notebook starts empty and says what to do');
    await axe('Statistics, before any test');
  });

  await S('the test chooser offers only the tests that are valid for the variables and the groups ticked', async () => {
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'nothing can be run before variables are chosen');
    h.check((await text(page.getByTestId('stats-why'))).includes('Choose the variables first'), 'and it says why');
    await goal('compare_groups');
    await pick('outcome', 'Stat score (scale)');
    await pick('group', 'Stat group (nominal)');
    await page.getByTestId('stats-methods').waitFor();
    let methods = await text(page.getByTestId('stats-methods'));
    h.check(methods.includes('One-way analysis of variance') && methods.includes('Kruskal-Wallis test') && !methods.includes('t test'), `three groups offer the analysis of variance and Kruskal-Wallis: ${methods}`);
    await page.getByTestId('stats-group-C').click();
    methods = await text(page.getByTestId('stats-methods'));
    h.check(methods.includes('Independent-samples t test') && methods.includes('Mann-Whitney U test') && !methods.includes('Kruskal'), `two groups ticked offer the t test and Mann-Whitney: ${methods}`);
    await page.getByTestId('stats-group-B').click();
    h.check((await text(page.getByTestId('stats-why'))).includes('Tick at least two groups'), 'one group left says to tick two');
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'and nothing can be run');
    await page.getByTestId('stats-group-B').click();
    await page.getByTestId('stats-group-C').click();
    // A nominal variable is not a number to compare; the list of numbers does not hold it.
    const options = await page.getByTestId('stats-role-outcome').locator('option').allInnerTexts();
    h.check(!options.some((o) => o.startsWith('Stat group')) && options.some((o) => o.startsWith('Stat age')), 'only numbers are offered as the number to compare');
  });

  await S('an analysis of variance adds a page: tables, a results sentence, a reading, the conditions it depends on, and the cases behind', async () => {
    await page.getByTestId('stats-method-one_way_anova').check();
    await run();
    const p = lastPage();
    const title = await text(p.getByTestId('stats-title'));
    h.check(title.includes('One-way analysis of variance: Stat score by Stat group'), `the page is titled by the test and the variables: ${title}`);
    const sentence = await text(p.getByTestId('stats-sentence'));
    h.check(/differed significantly between the 3 groups of Stat group: F\(2, 9\) = 60\.00, p < \.001, η² = \.93\./.test(sentence), `the results sentence is APA-style: ${sentence}`);
    h.equal(await p.getByTestId('stats-table').count(), 3, 'three tables: the groups, the analysis of variance, and the pairs');
    const anova = await text(p.getByTestId('stats-table').nth(1));
    h.check(/Between groups 200\.00 2 100\.00 60\.00 < ?\.001|Between groups 200\.00 2 100\.00 60\.00 \.000/.test(anova) || anova.includes('Between groups 200.00 2 100.00 60.00'), `the table of the analysis of variance: ${anova}`);
    const pairs = await text(p.getByTestId('stats-table').nth(2));
    h.check(pairs.includes('adjusted by Holm') && pairs.includes('A − B') && pairs.includes('A − C') && pairs.includes('B − C'), `every pair is compared and the adjustment is named: ${pairs}`);
    h.check((await text(p.getByTestId('stats-reading'))).includes('large effect by the usual benchmarks'), 'a plain reading says how large the effect is');
    const kinds = await p.getByTestId('stats-chart').evaluateAll((els) => els.map((e) => e.getAttribute('data-chart')));
    h.equal(kinds.join(','), 'box,means', 'a box plot of the groups and the means with their intervals');
    await shoot(p.getByTestId('stats-chart').first(), 'box');
    await shoot(p.getByTestId('stats-chart').nth(1), 'means');
    const checks = await text(p.getByTestId('stats-assumptions'));
    h.check(checks.includes('Codings are never rows') && /Design/.test(checks), 'the design rule is stated');
    h.check(/Fine|Note|Problem/.test(checks), 'each check carries a word for its state, not a color alone');
    h.check(/Levene's test|spread/.test(checks), 'equal spread is checked');
    const focused = await page.evaluate(() => document.activeElement?.getAttribute('data-testid'));
    h.equal(focused, 'stats-title', 'the new page takes the focus, so a keyboard user lands on the result');
    await p.getByText('The cases behind each group').click();
    h.check((await text(p.locator('details.stats-cases'))).includes('R1'), 'the cases in each group can be listed');
    await axe('Statistics, an analysis of variance');
  });

  await S('the pairs after an analysis of variance can be compared by Tukey, Games-Howell, or Scheffe, and the normality check carries the Shapiro-Wilk test', async () => {
    await goal('compare_groups');
    await pick('outcome', 'Stat score (scale)');
    await pick('group', 'Stat group (nominal)');
    await page.getByTestId('stats-method-one_way_anova').check();
    const choices = await page.getByTestId('stats-adjust').locator('option').allInnerTexts();
    h.check(choices.some((c) => c.startsWith("Tukey's")) && choices.some((c) => c.startsWith('Games-Howell')) && choices.some((c) => c.startsWith('Scheff')) && choices.some((c) => c.startsWith('Holm')), `three tests made for pairs and the adjusted t tests are offered: ${choices.join(' | ')}`);
    await page.getByTestId('stats-adjust').selectOption('tukey');
    await run();
    let p = lastPage();
    let pairs = await text(p.locator('.table-wrap').nth(2));
    h.check(pairs.includes("Pairs of groups, Tukey's honestly significant difference") && pairs.includes('A − B') && !pairs.includes('Adjusted p'), `Tukey's test names itself and gives one p-value: ${pairs}`);
    h.check(/hold for all the pairs together/.test(pairs), 'and says its intervals hold for all the pairs together');
    // The groups are A 10 12 11 13, B 15 17 16 18, C 20 22 21 23: the pooled variance is 15 / 9, so A minus B is -5.00 with a t of -5.48 on 9 degrees of freedom.
    h.check(/A − B -5\.00 \[-7\.\d\d, -2\.\d\d\] -5\.48 9 \.00\d/.test(pairs), `A minus B, worked out by hand: ${pairs}`);
    const checks = await text(p.getByTestId('stats-assumptions'));
    h.check(/Shapiro-Wilk test: W = \.\d{3}, p = \.\d{3}\./.test(checks), `the normality check shows the Shapiro-Wilk test: ${checks.slice(0, 300)}`);
    h.check(/read it with the shape and the plot/.test(checks), 'and says how to read it');
    await page.getByTestId('stats-adjust').selectOption('games_howell');
    await run();
    pairs = await text(lastPage().locator('.table-wrap').nth(2));
    h.check(pairs.includes('Pairs of groups, Games-Howell') && /does not assume equal spread/.test(pairs), `Games-Howell says it does not assume equal spread: ${pairs}`);
    await page.getByTestId('stats-adjust').selectOption('scheffe');
    await run();
    pairs = await text(lastPage().locator('.table-wrap').nth(2));
    h.check(pairs.includes("Scheffé's test") && /every comparison of groups/.test(pairs), `Scheffe's test says it holds for every comparison: ${pairs}`);
    await page.getByTestId('stats-adjust').selectOption('holm');
    await page.getByTestId('stats-method-kruskal_wallis').check();
    const kw = await page.getByTestId('stats-adjust').locator('option').allInnerTexts();
    h.check(!kw.some((c) => c.startsWith("Tukey's")) && kw.some((c) => c.startsWith('Holm')), `after Kruskal-Wallis only the adjustments are offered: ${kw.join(' | ')}`);
    await axe('Statistics, pairs compared by Scheffe');
  });

  await S('a t test of two chosen groups is Welch\'s by default, and Student\'s when equal variances are assumed', async () => {
    await page.getByTestId('stats-group-C').click();
    await page.getByTestId('stats-method-independent_t').check();
    await run();
    let sentence = await text(lastPage().getByTestId('stats-sentence'));
    h.check(/^Mean Stat score was lower in A \(M = 11\.50, SD = 1\.29, n = 4\) than in B \(M = 16\.50, SD = 1\.29, n = 4\)\. The difference was statistically significant, Welch t\(6\.00\) = -5\.48, p = \.002, d = -3\.87/.test(sentence), `Welch's test as a report would write it: ${sentence}`);
    h.check(/^Page \d+: Welch's t test/.test(await text(lastPage().getByTestId('stats-title'))), 'pages are numbered');
    await page.getByTestId('stats-equal-variances').click();
    await run();
    sentence = await text(lastPage().getByTestId('stats-sentence'));
    h.check(/Student's|t\(6\) = -5\.48/.test(sentence) && !sentence.includes('Welch'), `the pooled test when asked: ${sentence}`);
    h.check((await text(lastPage().getByTestId('stats-title'))).includes("Student's t test"), 'and the title says so');
    await page.getByTestId('stats-equal-variances').click();
    await page.getByTestId('stats-method-mann_whitney').check();
    await run();
    const mw = await text(lastPage().getByTestId('stats-sentence'));
    h.check(/ranked lower in A .* than in B .*U = 0\.0, p = \.0\d\d/.test(mw), `a rank-based test of the same groups: ${mw}`);
    const note = await text(lastPage().getByTestId('stats-table').last());
    h.check(note.includes('exact'), 'it says the p-value is exact for so few cases');
  });

  await S('a chi-square test of two categories counts cases, shows the residuals, and says when the counts are too small to trust', async () => {
    await goal('association');
    await pick('row', 'Stat group (nominal)');
    await pick('column', 'Stat site (nominal)');
    await page.getByTestId('stats-method-chi_square').check();
    await run();
    const p = lastPage();
    const sentence = await text(p.getByTestId('stats-sentence'));
    h.check(/^Stat group and Stat site were not statistically associated, χ²\(2, N = 12\) = 0\.00, p > \.999, Cramér's V = \.00\./.test(sentence), `the sentence: ${sentence}`);
    h.equal(await p.getByTestId('stats-table').count(), 3, 'the counts, the test, and the adjusted residuals');
    const counts = await text(p.getByTestId('stats-table').first());
    h.check(counts.includes('A 2 2 4') && counts.includes('Total 6 6 12'), `the table of cases with its totals: ${counts}`);
    const problem = p.locator('[data-status=problem]');
    h.check((await text(problem)).includes('The chi-square approximation is unreliable'), 'every expected count is under 5, and that is called a problem in words');
    h.check((await text(p.getByTestId('stats-reading'))).includes('do not describe it as marginal'), 'a result that is not significant is not called marginal');
    const chart = p.getByTestId('stats-chart');
    h.equal(await chart.count(), 1, 'one chart: the stacked bars of the table');
    h.equal(await chart.getAttribute('data-chart'), 'stacked', 'stacked bars');
    const legend = await text(chart.getByTestId('chart-legend'));
    h.check(legend.includes('N') && legend.includes('S') && (await chart.getByTestId('chart-legend').locator('li').count()) === 2, `a legend names each color: ${legend}`);
    h.equal(await chart.locator('rect.chart-seg').count(), 6, 'a segment for each cell of the table');
    await shoot(chart, 'stacked');
  });

  await S('a correlation, a description of a number, and a description of categories', async () => {
    await goal('correlate');
    await pick('x', 'Stat score (scale)');
    await pick('y', 'Stat age (scale)');
    await page.getByTestId('stats-method-pearson').check();
    await run();
    const sentence = await text(lastPage().getByTestId('stats-sentence'));
    h.check(/relation between Stat score and Stat age was .*r\(10\) = /.test(sentence), `the correlation sentence: ${sentence}`);
    // Spearman's correlation of the same twelve pairs, which have no ties: rho .657 and R's p-value of .024 (series approximation to the exact test).
    await page.getByTestId('stats-method-spearman').check();
    await run();
    const sp = await text(lastPage().locator('.table-wrap').first());
    h.check(/\.66 12 exact \(series\) \.024/.test(sp) && sp.includes('series approximation to the exact test that R uses'), `Spearman's p-value is R's, and the page says how it was found: ${sp}`);
    await page.getByTestId('stats-method-pearson').check();

    await goal('describe');
    await pick('x', 'Stat age (scale)');
    await run();
    const d = await text(lastPage().getByTestId('stats-table').first());
    h.check(d.includes('Cases with a value 12') && d.includes('M 37.67'), `a number is described: ${d}`);
    await pick('x', 'Stat site (nominal)');
    await run();
    const f = await text(lastPage().getByTestId('stats-table').first());
    h.check(f.includes('Frequencies of Stat site') && /N 6 [\d.]+% 50\.0%/.test(f) && /S 6 [\d.]+% 50\.0%/.test(f), `categories are counted, with their share of the cases that have a value: ${f}`);
  });

  await S('a regression: predictors are added, a category gets a reference, steps show what each adds, and the page reports the models, the coefficients, and the checks', async () => {
    await goal('predict');
    await pick('outcome', 'Stat score (scale)');
    await page.getByTestId('stats-predictor-0').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-add-predictor').click();
    await page.getByTestId('stats-predictor-1').selectOption({ label: 'Stat group (nominal)' });
    h.equal(await page.getByTestId('stats-ref-1').inputValue(), 'A', 'the first category is the reference to begin with');
    await page.getByTestId('stats-use-steps').click();
    await page.getByTestId('stats-step-1').selectOption('2');
    await page.getByTestId('stats-method-linear_regression').check();
    await run();
    const p = lastPage();
    h.check((await text(p.getByTestId('stats-title'))).includes('Linear regression: Stat score from Stat age, Stat group'), 'the page is titled by the outcome and the predictors');
    const titles = await p.getByTestId('stats-table').locator('caption').allInnerTexts();
    h.check(titles.includes('Model summary for Stat score') && titles.includes('Coefficients, model 1') && titles.includes('Coefficients, model 2'), `a summary, and coefficients for each step: ${titles.join(' | ')}`);
    const summary = await text(p.getByTestId('stats-table').first());
    h.check(/1 \.42 \.36 7\.26 1, 10 \.023/.test(summary), `step 1 as R fits it (R squared .42, F(1, 10) = 7.26): ${summary}`);
    h.check(/2 \.97 \.96 91\.42 3, 8 < \.001 \.55 77\.79 < \.001/.test(summary), `step 2 and what it adds (R squared .97, change .55, F = 77.79): ${summary}`);
    const sentence = await text(p.getByTestId('stats-sentence'));
    h.check(sentence.startsWith('The model with Stat age, Stat group explained 97.2% of the variance in Stat score (adjusted R\u00b2 = .96), F(3, 8) = 91.42, p < .001.'), `the sentence gives the whole model first: ${sentence}`);
    const second = await text(p.getByTestId('stats-table').nth(2));
    h.check(second.includes('Stat group: B (compared with A)') && second.includes('Stat group: C (compared with A)'), `a category is named against its reference: ${second}`);
    const checks = await text(p.getByTestId('stats-assumptions'));
    h.check(checks.includes('Fewer than 5 cases for each predictor term') && checks.includes('variance inflation factor'), `the checks of a model are listed, and twelve cases for three terms is called a problem: ${checks}`);
    await axe('Statistics, a regression');
    await page.getByTestId('stats-ref-1').selectOption('B');
    await run();
    const flipped = await text(lastPage().getByTestId('stats-table').nth(2));
    h.check(flipped.includes('Stat group: A (compared with B)'), 'another reference category can be chosen');
  });

  await S('a logistic regression models one value of a yes-or-no outcome, and reports the odds ratios, how well the model fits, and how well it classifies', async () => {
    await goal('predict_binary');
    await pick('outcome', 'Stat site (nominal)');
    await page.getByTestId('stats-predictor-0').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-method-logistic_regression').check();
    h.equal(await page.getByTestId('stats-event').inputValue(), 'S', 'the later of the two values is modelled to begin with');
    await run();
    const p = lastPage();
    h.check((await text(p.getByTestId('stats-title'))).includes('Logistic regression: Stat site from Stat age'), 'the page is titled by the outcome and the predictors');
    const titles = await p.getByTestId('stats-table').locator('caption').allInnerTexts();
    const wanted = ['Model fit for Stat site', 'How well the last model fits and predicts', 'Observed and predicted outcomes', 'Hosmer-Lemeshow test of fit', 'Coefficients', 'Each variable left out'];
    h.check(wanted.every((x) => titles.includes(x)), `the fit, the prediction, the classification, the Hosmer-Lemeshow test, the coefficients, and each variable left out: ${titles.join(' | ')}`);
    const coefficients = await text(p.getByTestId('stats-table').filter({ hasText: 'Odds ratio' }).first());
    h.check(coefficients.includes('Odds ratio') && coefficients.includes('(Wald)') && coefficients.includes('(profile)') && coefficients.includes('Intercept'), `the odds ratios come with a Wald and a profile-likelihood interval: ${coefficients}`);
    const sentence = await text(p.getByTestId('stats-sentence'));
    h.check(sentence.startsWith('The model of Stat site (S against N) with Stat age ') && sentence.includes('Each extra unit of Stat age multiplied the odds of S by'), `the sentence names the value modelled and the odds ratio of each predictor: ${sentence}`);
    const checks = await text(p.getByTestId('stats-assumptions'));
    h.check(checks.includes('cases of the rarer outcome for each predictor term'), `six cases of the rarer outcome are called few: ${checks}`);
    await axe('Statistics, a logistic regression');

    // The other value can be modelled: the odds ratios turn over.
    await page.getByTestId('stats-event').selectOption('N');
    await run();
    h.check((await text(lastPage().getByTestId('stats-sentence'))).startsWith('The model of Stat site (N against S) with Stat age '), 'choosing the other value models the odds of that value');

    // A category as a second predictor, entered in a second step: a model for each step and the change between them.
    await page.getByTestId('stats-add-predictor').click();
    await page.getByTestId('stats-predictor-1').selectOption({ label: 'Stat group (nominal)' });
    const steps = page.getByTestId('stats-use-steps');
    if (!(await steps.isChecked())) await steps.click();
    await page.getByTestId('stats-step-1').selectOption('2');
    await run();
    const stepped = await lastPage().getByTestId('stats-table').locator('caption').allInnerTexts();
    h.check(stepped.includes('Coefficients, model 1') && stepped.includes('Coefficients, model 2'), `a table of coefficients for each step: ${stepped.join(' | ')}`);
    const fit = await text(lastPage().getByTestId('stats-table').first());
    h.check(fit.includes('χ² change') && fit.includes('p of the change'), `what the second step adds is in the table of fit: ${fit}`);
    const blocks = await text(lastPage().getByTestId('stats-table').filter({ hasText: 'Each variable left out' }));
    h.check(blocks.includes('Stat group') && blocks.includes('Stat age'), `each variable is left out in turn, a category with all its columns: ${blocks}`);

    // A bootstrap adds intervals for the coefficients and the area under the curve.
    await page.getByTestId('stats-bootstrap-on').click();
    await page.getByTestId('stats-resamples').selectOption('1000');
    await page.getByTestId('stats-seed').fill('5');
    await run();
    const boot = await text(lastPage().locator('.table-wrap').filter({ hasText: 'Bootstrap intervals' }));
    h.check(boot.includes('Area under the curve') && boot.includes('B for Stat age') && boot.includes('1,000 resamples of the cases, from seed 5'), `bootstrap intervals for the coefficients and the area under the curve: ${boot}`);
    await page.getByTestId('stats-bootstrap-on').click();
  });

  await S('a curve fit compares the curves, draws them through the points, and keeps a table of what each curve is at every case', async () => {
    await goal('curve');
    await pick('outcome', 'Stat score (scale)');
    await pick('predictor', 'Stat age (scale)');
    await page.getByTestId('stats-method-curve_fit').check();
    h.equal(await page.getByTestId('stats-curve-options').locator('input[type=checkbox]').count(), 8, 'the eight curves can be ticked');
    await run();
    const p = lastPage();
    h.check((await text(p.getByTestId('stats-title'))).includes('Curve fit: Stat score against Stat age'), 'the page is titled by the two numbers');
    const titles = await p.getByTestId('stats-table').locator('caption').allInnerTexts();
    h.check(titles.includes('Curves fitted to Stat score against Stat age') && titles.includes('Coefficients: Linear') && titles.includes('Coefficients: S curve'), `a comparison and the coefficients of each curve: ${titles.join(' | ')}`);
    const compare = await text(p.getByTestId('stats-table').first());
    h.check(compare.includes('Equation') && compare.includes('Quadratic') && compare.includes('ln(Stat score)'), `the comparison names each curve, what it was fitted on, and its equation: ${compare}`);
    const sentence = await text(p.getByTestId('stats-sentence'));
    h.check(sentence.startsWith('Of the 8 curves fitted to Stat score against Stat age, the '), `the sentence names the curve that came closest: ${sentence}`);
    const figure = p.locator('[data-chart=curves]');
    h.equal(await figure.count(), 1, 'one chart draws the points and the curves');
    h.equal(await figure.locator('polyline').count(), 8, 'a line for each curve');
    h.equal(await figure.locator('[data-testid=chart-legend] li').count(), 8, 'and a legend entry for each, with its line style');
    const alt = await text(figure.locator('figcaption'));
    h.check(alt.includes('with 8 curves fitted: Linear, Logarithmic, Inverse, Quadratic, Cubic, Power, Exponential, S curve'), `the text alternative names the curves: ${alt}`);
    await figure.locator('summary').click();
    const grid = await text(figure.locator('table').first());
    await figure.locator('summary').click();
    h.check(grid.includes('Quadratic (fitted)') && grid.includes('Power (fitted)'), `the table of the chart has a column for each curve: ${grid.slice(0, 160)}`);
    await shoot(figure, 'curves');
    await axe('Statistics, a curve fit');

    // Fewer curves: untick two, and the page and the chart have six.
    await page.getByTestId('stats-curve-cubic').click();
    await page.getByTestId('stats-curve-s_curve').click();
    await run();
    h.equal(await lastPage().locator('[data-chart=curves] polyline').count(), 6, 'only the curves ticked are fitted and drawn');
    h.check((await text(lastPage().getByTestId('stats-sentence'))).startsWith('Of the 6 curves fitted'), 'and counted');
    // At least one stays ticked.
    for (const k of ['linear', 'logarithmic', 'inverse', 'quadratic', 'power', 'exponential']) await page.getByTestId(`stats-curve-${k}`).click();
    h.equal(await page.getByTestId('stats-curve-options').locator('input[type=checkbox]:checked').count(), 1, 'the last curve cannot be unticked');
    for (const k of ['linear', 'logarithmic', 'inverse', 'quadratic', 'cubic', 'power', 's_curve']) {
      if (!(await page.getByTestId(`stats-curve-${k}`).isChecked())) await page.getByTestId(`stats-curve-${k}`).click();
    }
  });

  await S('bootstrap intervals are added by a tick, repeat for a saved seed, and change with another seed', async () => {
    await goal('describe');
    await pick('x', 'Stat age (scale)');
    await page.getByTestId('stats-bootstrap-on').click();
    await page.getByTestId('stats-resamples').selectOption('1000');
    await page.getByTestId('stats-seed').fill('42');
    await run();
    const bootTable = (pg) => text(pg.locator('.table-wrap').filter({ hasText: 'Bootstrap intervals' }));
    const first = await bootTable(lastPage());
    h.check(first.includes('Bootstrap intervals') && first.includes('Mean') && first.includes('Median') && first.includes('Standard deviation'), `a bootstrap table for the mean, the median, and the SD: ${first}`);
    h.check(first.includes('1,000 resamples of the cases, from seed 42'), 'it says how many resamples and which seed');
    await run();
    h.equal(await bootTable(lastPage()), first, 'the same seed gives the same intervals');
    await page.getByTestId('stats-seed').fill('43');
    await run();
    h.check((await bootTable(lastPage())) !== first, 'another seed gives other intervals');

    await goal('predict');
    await pick('outcome', 'Stat score (scale)');
    await page.getByTestId('stats-predictor-0').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-method-linear_regression').check();
    await run();
    const reg = await bootTable(lastPage());
    h.check(reg.includes('Intercept') && reg.includes('B for Stat age') && reg.includes('R\u00b2') && reg.includes('BCa'), `the coefficients and R squared are bootstrapped: ${reg}`);
    h.check(/\[[\d.-]+, [\d.-]+\]/.test(reg), 'with intervals');
    await axe('Statistics, a bootstrap');
    await page.getByTestId('stats-bootstrap-on').click();
  });

  await S('the whole analysis can be bootstrapped for any test, with single cases or with whole groups of cases kept together', async () => {
    const wholeTable = (pg) => text(pg.locator('.table-wrap').filter({ hasText: 'Bootstrap of the whole analysis' }));
    await goal('compare_groups');
    await pick('outcome', 'Stat score (scale)');
    await pick('group', 'Stat group (nominal)');
    await page.getByTestId('stats-method-one_way_anova').check();
    await page.getByTestId('stats-bootstrap-on').click();
    h.equal(await page.getByTestId('stats-bootstrap-what').count(), 0, 'a test with no statistic of its own is not asked what to bootstrap');
    h.check((await text(page.getByTestId('stats-bootstrap'))).includes('has no single statistic of its own'), 'and it says that the whole analysis is run again');
    await page.getByTestId('stats-resamples').selectOption('1000');
    await page.getByTestId('stats-seed').fill('11');
    await run();
    const anova = await wholeTable(lastPage());
    h.check(anova.includes('Resamples with p < .05') && anova.includes('F') && anova.includes('\u03b7\u00b2') && anova.includes('1,000 resamples of the cases, drawn with replacement, from seed 11'), `the numbers of the analysis of variance each have their spread: ${anova.slice(0, 400)}`);
    h.check(/\d+(\.\d)?%/.test(anova), 'and the share of resamples in which p fell below .05');
    h.equal(await lastPage().locator('.table-wrap').filter({ hasText: 'Bootstrap intervals' }).count(), 0, 'it is the whole analysis and not the one statistic');

    // A test that has a statistic of its own is asked which to bootstrap.
    await page.getByTestId('stats-group-C').click();
    await page.getByTestId('stats-method-independent_t').check();
    h.equal(await page.getByTestId('stats-bootstrap-what').count(), 1, 'a t test can be bootstrapped either way');
    await page.getByTestId('stats-bootstrap-whole').check();
    await run();
    const t1 = await wholeTable(lastPage());
    h.check(t1.includes('Difference of means') && t1.includes("Cohen's d"), `the t test is run again on each resample: ${t1.slice(0, 300)}`);
    await run();
    h.equal(await wholeTable(lastPage()), t1, 'the same seed gives the same intervals');

    // Whole groups of cases kept together: the six teams.
    const options = await page.getByTestId('stats-cluster').locator('option').allInnerTexts();
    h.check(options.some((o) => o.includes('Stat team') && o.includes('6 groups')) && !options.some((o) => o.includes('Stat site')), `only a variable with enough groups can be resampled by: ${options.join(' | ')}`);
    await page.getByTestId('stats-cluster').selectOption({ label: options.find((o) => o.includes('Stat team')) });
    await run();
    const teams = await wholeTable(lastPage());
    h.check(/1,000 resamples of the \d+ groups of cases that share a value of Stat team, drawn with replacement and kept together, from seed 11/.test(teams), `the groups are said (a case with no team is a group of its own, so other cases in the project add groups): ${teams.slice(-420)}`);
    h.check(teams !== t1, 'keeping the teams together changes the intervals');
    await axe('Statistics, a bootstrap of the whole analysis');

    // Put the choices back.
    await page.getByTestId('stats-cluster').selectOption('');
    await page.getByTestId('stats-bootstrap-statistic').check();
    await page.getByTestId('stats-bootstrap-on').click();
    await page.getByTestId('stats-group-C').click();
  });

  await S('a description draws a histogram and a normal probability plot, each with a text alternative, a readout, and the numbers as a table', async () => {
    await goal('describe');
    await pick('x', 'Stat age (scale)');
    await page.getByTestId('stats-bootstrap-on').click();
    await run();
    const p = lastPage();
    const charts = p.getByTestId('stats-chart');
    h.equal(await charts.count(), 2, 'a histogram and a normal probability plot');
    const hist = charts.first();
    h.check((await text(hist.locator('h6'))) === 'Histogram of Stat age', 'the chart has a title');
    const description = await text(hist.locator('figcaption p'));
    h.check(/^12 cases in \d+ bars, from \d+ to \d+\. The tallest bar, from \d+ to \d+, holds \d+ cases?\. The mean is 37\.67\./.test(description), `a text alternative says what the chart shows: ${description}`);
    const bars = hist.locator('rect.chart-mark');
    h.check((await bars.count()) >= 3, 'the bars are drawn');
    await bars.first().focus();
    const readout = await text(hist.getByTestId('chart-readout'));
    h.check(/^\d+ to \d+: \d+ cases?$/.test(readout), `a bar shows its numbers when it has the focus: ${readout}`);
    await hist.locator('summary').click();
    const rows = await hist.locator('tbody tr').count();
    h.equal(rows, await hist.locator('svg').evaluate((s) => s.querySelectorAll('rect.chart-mark').length), 'the table has a row for each bar');
    const qq = charts.nth(1);
    h.check((await text(qq.locator('h6'))) === 'Normal probability plot of Stat age', 'the second chart is the normal probability plot');
    h.equal(await qq.locator('circle.chart-point').count(), 12, 'a point for each of the twelve ages');
    h.equal(await qq.locator('svg').getAttribute('role'), 'img', 'a chart with no keyboard marks is one picture with a label');
    await shoot(hist, 'histogram');
    await shoot(qq, 'qq');
    await axe('Statistics, charts of a description');

    await goal('predict');
    await pick('outcome', 'Stat score (scale)');
    await page.getByTestId('stats-predictor-0').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-method-linear_regression').check();
    await run();
    const reg = lastPage().getByTestId('stats-chart');
    const kinds = await reg.evaluateAll((els) => els.map((e) => e.getAttribute('data-chart')));
    h.equal(kinds.join(','), 'scatter,qq,scatter', 'residuals against fitted values, their normal plot, and the data with the line');
    h.check((await text(reg.first().locator('h6'))).startsWith('Residuals against') || (await text(reg.first().locator('h6'))).includes('against'), 'the first is the residual plot');
    h.equal(await reg.nth(2).locator('line.chart-line').count(), 1, 'the line the model fitted is drawn through the data');
    await shoot(reg.first(), 'residuals');
    await shoot(reg.nth(2), 'regression-line');
    await axe('Statistics, charts of a regression');
  });

  // ---- accessibility of the charts, beyond what axe can see: contrast of what is drawn, the keyboard, the text size, narrow windows

  /** The contrast of what is drawn, measured on the screen in the colors now in use. */
  const measureChartColors = () =>
    page.evaluate(() => {
      const parse = (c) => {
        const m = c && c.match(/rgba?\(([^)]+)\)/);
        if (!m) return null;
        const [r, g, b, a = 1] = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
        return { r, g, b, a };
      };
      const lum = ({ r, g, b }) => {
        const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
        return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
      };
      const ratio = (a, b) => {
        const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
        return (x + 0.05) / (y + 0.05);
      };
      const panel = parse(getComputedStyle(document.querySelector('.stats-page-card')).backgroundColor);
      const over = (c, alpha) => ({ r: c.r * alpha + panel.r * (1 - alpha), g: c.g * alpha + panel.g * (1 - alpha), b: c.b * alpha + panel.b * (1 - alpha) });
      const worst = (selector, prop) => {
        let min = Infinity;
        let seen = 0;
        for (const el of document.querySelectorAll(selector)) {
          const cs = getComputedStyle(el);
          const c = parse(cs[prop]);
          if (!c || c.a === 0) continue;
          const opacity = prop === 'fill' ? parseFloat(cs.fillOpacity) * c.a : c.a;
          min = Math.min(min, ratio(over(c, opacity), panel));
          seen++;
        }
        return { min: seen ? Math.round(min * 100) / 100 : null, seen };
      };
      // What a segment of stacked bars has to tell it from its neighbours and the page: its fill, or failing that its outline.
      const segments = [...document.querySelectorAll('rect.chart-seg')].map((el) => {
        const cs = getComputedStyle(el);
        const fill = ratio(parse(cs.fill), panel);
        const outline = parseFloat(cs.strokeWidth) >= 1 ? ratio(parse(cs.stroke), panel) : 0;
        return Math.max(fill, outline);
      });
      const pills = [...document.querySelectorAll('.chart-seg-label')].map((el) => {
        const bg = el.previousElementSibling && getComputedStyle(el.previousElementSibling).fill;
        return bg ? ratio(parse(getComputedStyle(el).fill), parse(bg)) : 0;
      });
      return {
        text: worst('.chart-text', 'fill'),
        muted: worst('.chart-muted', 'fill'),
        axis: worst('.chart-axis', 'stroke'),
        bars: worst('.chart-mark', 'fill'),
        points: worst('.chart-point', 'fill'),
        box: worst('.chart-box', 'stroke'),
        whisker: worst('.chart-whisker', 'stroke'),
        median: worst('.chart-median', 'stroke'),
        line: worst('.chart-line', 'stroke'),
        reference: worst('.chart-ref', 'stroke'),
        segments: segments.length ? Math.round(Math.min(...segments) * 100) / 100 : null,
        segmentLabels: pills.length ? Math.round(Math.min(...pills) * 100) / 100 : null,
        legend: [...document.querySelectorAll('.chart-swatch')].length,
      };
    });
  const checkColors = async (where) => {
    const c = await measureChartColors();
    const detail = JSON.stringify(c);
    h.check(c.text.min >= 4.5 && c.muted.min >= 4.5, `${where}: the words on charts have at least 4.5 to 1: ${detail}`);
    for (const k of ['axis', 'bars', 'points', 'box', 'whisker', 'median', 'line', 'reference']) {
      if (c[k].seen > 0) h.check(c[k].min >= 3, `${where}: ${k} has at least 3 to 1 against the page: ${c[k].min}`);
    }
    h.check(c.segments === null || c.segments >= 3, `${where}: each segment of the stacked bars is told from the page by its fill or its outline at 3 to 1: ${c.segments}`);
    h.check(c.segmentLabels === null || c.segmentLabels >= 4.5, `${where}: the share written in a segment has at least 4.5 to 1 against what is behind it: ${c.segmentLabels}`);
  };

  await S('what charts draw keeps its contrast in the light, dark, and high contrast themes, and holds up under forced colors', async () => {
    await checkColors('light');
    await page.emulateMedia({ colorScheme: 'dark' });
    await checkColors('dark');
    await axe('Statistics charts, dark theme');
    await page.evaluate(() => { document.documentElement.dataset.theme = 'contrast'; });
    await checkColors('high contrast');
    await axe('Statistics charts, high contrast theme');
    await page.evaluate(() => { delete document.documentElement.dataset.theme; });
    await page.emulateMedia({ colorScheme: 'light', forcedColors: 'active' });
    const drawn = await page.locator('svg.stats-svg').count();
    h.check(drawn > 5, `the charts are still drawn under forced colors: ${drawn}`);
    await axe('Statistics charts, forced colors');
    await page.emulateMedia({ forcedColors: 'none' });
  });

  await S('every mark of a chart can be reached from the keyboard, shows where the focus is, and the focus is not trapped', async () => {
    const fig = page.locator('[data-chart=box]').first();
    const marks = fig.locator('svg [tabindex="0"]');
    const n = await marks.count();
    h.check(n >= 2, `the groups of a box plot are keyboard stops: ${n}`);
    for (let i = 0; i < n; i++) {
      const label = await marks.nth(i).getAttribute('aria-label');
      h.check(label && label.length > 20, `stop ${i + 1} has a name that says its numbers: ${label}`);
    }
    await marks.first().focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    const focused = await page.evaluate(() => {
      const el = document.activeElement;
      const cs = getComputedStyle(el);
      return { label: el.getAttribute('aria-label'), outline: cs.outlineStyle, width: parseFloat(cs.outlineWidth), inFigure: !!el.closest('[data-chart=box]') };
    });
    h.check(focused.inFigure && focused.outline !== 'none' && focused.width >= 2, `the focused mark has a visible outline: ${JSON.stringify(focused)}`);
    h.check((await text(fig.getByTestId('chart-readout'))).length > 20, 'and its numbers are written under the chart');
    // Tab on through the chart: it leaves after the last mark.
    let left = false;
    for (let i = 0; i < n + 3 && !left; i++) {
      await page.keyboard.press('Tab');
      left = await page.evaluate(() => !document.activeElement?.closest('svg'));
    }
    h.check(left, 'Tab moves on past the last mark, so the keyboard is not trapped in the chart');
    h.check((await fig.locator('summary').count()) === 1, 'and the numbers behind the chart are a button away');
    await page.keyboard.press('Tab');
    const next = await page.evaluate(() => document.activeElement?.tagName);
    h.check(next !== undefined && next !== 'BODY' && next !== 'HTML', `focus lands on something that can be used: ${next}`);
  });

  await S('charts have a sensible heading outline and a name that is not repeated, and a chart with marks is a group of them', async () => {
    const outline = await page.evaluate(() => {
      const card = document.querySelector('[data-testid=stats-page][data-procedure=describe]') ?? document.querySelector('[data-testid=stats-page]');
      return [...card.querySelectorAll('h4,h5,h6')].map((el) => Number(el.tagName[1]));
    });
    let skipped = false;
    for (let i = 1; i < outline.length; i++) if (outline[i] - outline[i - 1] > 1) skipped = true;
    h.check(!skipped, `no heading level is skipped inside a page: ${outline.join(' ')}`);
    const roles = await page.locator('[data-chart=box] svg').first().getAttribute('role');
    h.equal(roles, 'group', 'a chart with keyboard marks is a labelled group');
    const names = await page.locator('figure.stats-chart h6').allInnerTexts();
    h.check(names.length > 5 && names.every((n) => n.length > 3), `each chart has its own heading: ${names.length}`);
  });

  await S('the text size setting scales the words in a chart and nothing is cut off', async () => {
    const gear = page.getByRole('banner').getByRole('button', { name: 'Settings', exact: true });
    const sizeOf = () => page.evaluate(() => parseFloat(getComputedStyle(document.querySelector('svg.stats-svg text.chart-text')).fontSize) * (document.querySelector('svg.stats-svg').getBoundingClientRect().width / document.querySelector('svg.stats-svg').viewBox.baseVal.width));
    const standard = await sizeOf();
    await gear.click();
    await page.getByRole('heading', { level: 2, name: 'Settings' }).waitFor();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^150 percent/ }) }).click();
    await open();
    await page.waitForFunction(() => document.querySelector('svg.stats-svg') !== null);
    await page.waitForTimeout(300);
    const big = await sizeOf();
    h.check(big >= standard * 1.35, `the words in a chart grew with the text size, ${standard.toFixed(1)} to ${big.toFixed(1)} px`);
    const cut = await page.evaluate(() => {
      const bad = [];
      for (const svg of document.querySelectorAll('svg.stats-svg')) {
        const frame = svg.getBoundingClientRect();
        for (const el of svg.querySelectorAll('text')) {
          const b = el.getBoundingClientRect();
          if (b.width > 0 && (b.left < frame.left - 1 || b.right > frame.right + 1 || b.top < frame.top - 1 || b.bottom > frame.bottom + 1)) bad.push(el.textContent);
        }
      }
      return bad;
    });
    h.equal(cut, [], `no word in a chart is cut off at 150 percent: ${cut.join(', ')}`);
    await axe('Statistics charts at 150 percent text');
    await gear.click();
    await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'Interface', exact: true }).click();
    await page.locator('label').filter({ has: page.getByRole('radio', { name: /^100 percent/ }) }).click();
    await open();
  });

  await S('in a narrow window the page reflows, and only a chart or a table scrolls sideways, inside its own box', async () => {
    const before = page.viewportSize();
    await page.setViewportSize({ width: 360, height: 740 });
    await page.waitForTimeout(400);
    const flow = await page.evaluate(() => ({ page: document.documentElement.scrollWidth - document.documentElement.clientWidth, charts: [...document.querySelectorAll('svg.stats-svg')].filter((s) => s.getBoundingClientRect().right > window.innerWidth + 1).length }));
    h.check(flow.page <= 1, `the page does not scroll sideways at 360 px wide: ${flow.page}`);
    h.equal(flow.charts, 0, 'no chart is wider than the window');
    await axe('Statistics charts in a narrow window');
    await page.setViewportSize(before);
  });

  // ---- the tests added after the first set; each number below was worked out in R 4.5.3 (or, for the runs test and the Moses test, by listing every arrangement of the twelve cases)

  const sentenceOf = async () => text(lastPage().getByTestId('stats-sentence'));
  // The tables and their notes; the numbers behind a chart are tables too, and are left out here.
  const tablesOf = async () => (await lastPage().locator('.table-wrap').evaluateAll((els) => els.filter((e) => !/numbers$/.test(e.getAttribute('aria-label') ?? '')).map((e) => e.innerText))).join(' ').replace(/\s+/g, ' ').trim();

  await S('a description can be tested for normality, for the order of its cases, for a share, and for equal shares', async () => {
    await goal('describe');
    await pick('x', 'Stat age (scale)');
    const offered = await text(page.getByTestId('stats-methods'));
    h.check(offered.includes('Shapiro-Wilk test of normality') && offered.includes('Kolmogorov-Smirnov test against a normal curve') && offered.includes('Runs test'), `a number is offered the tests of normality and of order: ${offered}`);
    h.check(!offered.includes('Binomial test'), 'a number has no categories to count');
    await page.getByTestId('stats-method-shapiro_wilk').check();
    await run();
    h.check(/did not depart from a normal distribution by the Shapiro-Wilk test, W\(12\) = \.980, p = \.985/.test(await sentenceOf()), `Shapiro-Wilk as R gives it (W .9804, p .9853): ${await sentenceOf()}`);
    h.check(/Shapiro-Wilk test of normality: Stat age/.test(await text(lastPage().getByTestId('stats-title'))), 'and the page is titled by the test');

    await page.getByTestId('stats-method-ks_normal').check();
    await page.getByTestId('stats-ks-mean').fill('37');
    await page.getByTestId('stats-ks-sd').fill('11');
    await run();
    const ks = await tablesOf();
    h.check(/The curve is the normal distribution with the mean and standard deviation given/.test(ks) && ks.includes('Mean of the curve') && !ks.includes('too large'), `a curve given is not called an estimate: ${ks}`);
    await page.getByTestId('stats-ks-sd').fill('');
    await run();
    h.check(/the mean and the standard deviation of the curve were taken from the sample, so this p-value is too large/i.test(await tablesOf()), 'a curve taken from the sample says its p-value is too large');

    await pick('x', 'Stat score (scale)');
    await page.getByTestId('stats-method-runs').check();
    await page.getByTestId('stats-cut').waitFor();
    await run();
    // Scores in the order of the cases cut at the median 16.5: F F F F F T F T T T T T, so 4 runs where 7 are expected (6 cases in each part); every arrangement listed gives an exact two-sided p of .134.
    h.check(/The order of Stat score did not differ from a random order: there were 4 runs where 7\.00 are expected, p = \.134/.test(await sentenceOf()), `the runs test: ${await sentenceOf()}`);
    h.check(/cut at the median, 16\.50/.test(await tablesOf()), 'and says where it cut the number');
    await page.getByTestId('stats-cut').selectOption('value');
    await page.getByTestId('stats-cut-value').fill('12.5');
    await run();
    h.check(/cut at the value, 12\.50/.test(await tablesOf()), 'a value to cut at can be given');

    await pick('x', 'Stat site (nominal)');
    const two = await text(page.getByTestId('stats-methods'));
    h.check(two.includes('Binomial test') && two.includes('One-sample chi-square test') && two.includes('Runs test'), `a variable with two values is offered the tests of counts and of order: ${two}`);
    await page.getByTestId('stats-method-binomial').check();
    await run();
    h.check(/^50\.0% of the cases \(6 of 12\) were N, which did not differ from 50\.0%, exact p > \.999, 95% CI \[0\.211, 0\.789\]\./.test(await sentenceOf()), `the binomial test (R: p 1, interval .2109 to .7891): ${await sentenceOf()}`);
    await page.getByTestId('stats-proportion').fill('0.8');
    await run();
    h.check(/which differed from 80\.0%, exact p = \.019/.test(await sentenceOf()), `another share to compare with (R: p .0194): ${await sentenceOf()}`);
    await page.getByTestId('stats-success').selectOption('S');
    await run();
    h.check(/were S, which/.test(await sentenceOf()), 'and the category to count can be chosen');
    await axe('Statistics, the binomial test and its options');

    await pick('x', 'Stat group (nominal)');
    await page.getByTestId('stats-method-goodness_of_fit').check();
    await run();
    h.check(/did not differ from equal shares, χ²\(2, N = 12\) = 0\.00, p > \.999/.test(await sentenceOf()), `four cases in each of three groups match equal shares: ${await sentenceOf()}`);
    await page.getByTestId('stats-shares-typed').check();
    await page.getByTestId('stats-share-A').fill('50');
    await page.getByTestId('stats-share-B').fill('30');
    await page.getByTestId('stats-share-C').fill('20');
    await run();
    // Expected 6, 3.6, 2.4: chi-square 1.78 on 2 degrees of freedom, p .411 (R), residuals -0.82, 0.21, 1.03.
    h.check(/did not differ from the expected shares, χ²\(2, N = 12\) = 1\.78, p = \.411/.test(await sentenceOf()), `typed shares: ${await sentenceOf()}`);
    const gof = await tablesOf();
    h.check(gof.includes('A 4 6.00 50.0% -0.82') && gof.includes('C 4 2.40 20.0% 1.03'), `expected counts and residuals as R gives them: ${gof}`);
    await axe('Statistics, a one-sample chi-square test with typed shares');
  });

  await S('the sign test, and the tests that compare two groups on more than their middles', async () => {
    await goal('against_value');
    await pick('x', 'Stat score (scale)');
    await page.getByTestId('stats-method-sign').check();
    await page.getByTestId('stats-mu').fill('15');
    await run();
    // Seven scores above 15, four below, one equal: R's binom.test(7, 11) gives p .5488 and the interval .3079 to .8907 for the share above.
    h.check(/^7 values of Stat score were above 15\.00 and 4 below, with 1 equal and set aside; by the sign test that did not differ from an even split, p = \.549/.test(await sentenceOf()), `the sign test: ${await sentenceOf()}`);
    h.check(/\[0\.308, 0\.891\]/.test(await tablesOf()), 'with the interval of the share that is above');

    await goal('compare_groups');
    await pick('outcome', 'Stat score (scale)');
    await pick('group', 'Stat site (nominal)');
    const offered = await text(page.getByTestId('stats-methods'));
    h.check(offered.includes('Kolmogorov-Smirnov test of two groups') && offered.includes("Mood's median test") && offered.includes('Moses test of extreme reactions'), `two groups are offered the tests of distributions, medians, and spread: ${offered}`);
    await page.getByTestId('stats-method-ks_two').check();
    await run();
    h.check(/The distributions of Stat score in N and S did not differ from each other, D = \.333, p = \.931/.test(await sentenceOf()), `the two-sample Kolmogorov-Smirnov test (R: D .3333, p .9307): ${await sentenceOf()}`);
    await page.getByTestId('stats-method-median_test').check();
    await run();
    h.check(/pooled median of Stat score \(16\.50\) did not differ between the groups of Stat site, χ²\(1, N = 12\) = 1\.33, p = \.248/.test(await sentenceOf()), `the median test (R: chi-square 1.3333, p .2482): ${await sentenceOf()}`);
    h.check(/Fisher's exact p/.test(await tablesOf()) && /\.567/.test(await tablesOf()), "and Fisher's exact p for two groups (R: .5671)");
    await page.getByTestId('stats-method-moses').check();
    await page.getByTestId('stats-control').waitFor();
    await run();
    h.check(/The control group, N, spanned 10 places in the pooled order of Stat score \(p = \.500\); with 1 trimmed from each end it spanned 8 \(p = \.879\)/.test(await sentenceOf()), `the Moses test (every arrangement of 12 cases listed: .500 and .879): ${await sentenceOf()}`);
    await page.getByTestId('stats-control').selectOption('S');
    await page.getByTestId('stats-trim').fill('0');
    await run();
    h.check(/The control group, S, spanned/.test(await sentenceOf()) && /with 0 trimmed from each end/.test(await sentenceOf()), `the control group and the trim can be set: ${await sentenceOf()}`);
    await axe('Statistics, the Moses test and its options');

    await pick('group', 'Stat group (nominal)');
    const three = await text(page.getByTestId('stats-methods'));
    h.check(three.includes("Mood's median test") && !three.includes('Moses') && !three.includes('Kolmogorov-Smirnov test of two groups'), `three groups keep only the median test of these three: ${three}`);
  });

  await S('repeated measures, an answer given twice, two ordered scales, and a correlation holding another number constant', async () => {
    await goal('repeated');
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'a repeated comparison cannot run before two measures are chosen');
    await page.getByTestId('stats-list-0').selectOption({ label: 'Stat score (scale)' });
    await page.getByTestId('stats-list-add').click();
    await page.getByTestId('stats-list-1').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-method-friedman').check();
    await run();
    // Every case has the score below the age, so the ranks agree completely: chi-square 12 on 1 degree of freedom (p .00053, R), Kendall's W 1.
    h.check(/^The 2 measures differed from one another in their ranks, χ²\(1, N = 12\) = 12\.00, p < \.001, Kendall's W = 1\.000/.test(await sentenceOf()), `Friedman's test: ${await sentenceOf()}`);
    const measures = await text(lastPage().getByTestId('stats-table').first());
    h.check(measures.includes('Stat score 1.00 16.50') && measures.includes('Stat age 2.00 37.00'), `mean ranks and medians of the measures: ${measures}`);

    await goal('association');
    await pick('row', 'Stat site (nominal)');
    await pick('column', 'Stat later (nominal)');
    const offered = await text(page.getByTestId('stats-methods'));
    h.check(offered.includes("McNemar's test"), `the same two answers given twice are offered McNemar's test: ${offered}`);
    await page.getByTestId('stats-method-mcnemar').check();
    await run();
    // One case changed from N to S and five from S to N: chi-square with the correction 1.5, p .2207, exact p .2188 (R).
    h.check(/^1 case changed from N to S and 5 from S to N; McNemar's χ²\(1\) = 1\.50, p = \.221 \(exact p = \.219\)/.test(await sentenceOf()), `McNemar's test: ${await sentenceOf()}`);

    await pick('row', 'Stat rate (ordinal)');
    await pick('column', 'Stat grade (ordinal)');
    const ordered = await text(page.getByTestId('stats-methods'));
    h.check(ordered.includes('Association between ordered categories') && !ordered.includes("McNemar's test"), `two ordered scales are offered the measures of order, and not McNemar's test (the values differ): ${ordered}`);
    await page.getByTestId('stats-method-ordinal_association').check();
    await run();
    // 44 of the 46 pairs that differ on both rank the same way: gamma 42/46 = .913, tau-b .778.
    h.check(/Stat rate and Stat grade were positively related by the order of their values, gamma = \.913, tau-b = \.778, p [<=] /.test(await sentenceOf()), `the measures of order: ${await sentenceOf()}`);
    h.check(/44 pairs of cases rank the same way on both variables and 2 rank opposite ways/.test(await tablesOf()), 'and the pairs behind them are counted');

    await goal('partial');
    await pick('x', 'Stat score (scale)');
    await pick('y', 'Stat age (scale)');
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'a partial correlation cannot run before something is held constant');
    await page.getByTestId('stats-list-0').selectOption({ label: 'Stat hours (scale)' });
    await page.getByTestId('stats-method-partial_correlation').check();
    await run();
    // R: the residuals of score and of age on hours correlate at -.3241 (t -1.03 on 9 degrees of freedom, p .331); without hours the two correlate at .648.
    const partial = await sentenceOf();
    h.check(/^The negative relation between Stat score and Stat age, holding constant Stat hours, was not statistically significant, partial r\(9\) = -\.32, p = \.331, 95% CI \[-0\.77, 0\.34\]/.test(partial), `the partial correlation: ${partial}`);
    h.check(/Without holding anything constant the correlation of the two is \.65\./.test(await tablesOf()), 'and the correlation without the control is beside it');
    h.check((await text(lastPage().getByTestId('stats-reading'))).includes('do not describe it as marginal'), 'a result that is not significant is not called marginal');
    await axe('Statistics, a partial correlation');
  });

  await S("a two-way analysis of variance and Cronbach's alpha are chosen, run, worded, and drawn", async () => {
    // The steps before may have left the bootstrap on; this step is about the analyses themselves.
    const bootstrapOn = await page.getByTestId('stats-bootstrap-on').evaluate((el) => (el.querySelector('input') ?? el).checked === true || el.getAttribute('aria-checked') === 'true').catch(() => false);
    if (bootstrapOn) await page.getByTestId('stats-bootstrap-on').click();
    await goal('two_factors');
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'a two-way analysis cannot run before the number and both groupings are chosen');
    await pick('outcome', 'Stat score (scale)');
    await pick('first', 'Stat group (nominal)');
    await pick('second', 'Stat site (nominal)');
    const offered = await text(page.getByTestId('stats-methods'));
    h.check(offered.includes('Two-way analysis of variance'), `two groupings are offered the two-way analysis: ${offered}`);
    await page.getByTestId('stats-method-two_way_anova').check();
    await run();
    // By hand and in R (drop1 with sum contrasts): 12 cases in six cells of two. Group SS 200 on 2 df, site SS 12 on 1, the interaction 0 on 2, the error 3 on 6
    // (MS .5), so F is 200 (p 3.2e-6), 24 (p .0027) and 0 (p 1); the partial eta squared are 200/203 and 12/15.
    const sentence = await sentenceOf();
    h.check(
      /^The main effect of Stat group was statistically significant, F\(2, 6\) = 200\.00, p < \.001, ηp² = \.985; The main effect of Stat site was statistically significant, F\(1, 6\) = 24\.00, p = \.003, ηp² = \.800; The interaction of Stat group and Stat site was not statistically significant, F\(2, 6\) = 0\.00, p > \.999/.test(sentence),
      `the two-way analysis as a report would write it: ${sentence}`,
    );
    const tables = await tablesOf();
    h.check(/Stat group 200\.00 2 100\.00 200\.00 < \.001 \.985/.test(tables) && /Error 3\.00 6 0\.50/.test(tables) && /Total 215\.00 11/.test(tables), `the table of the analysis: ${tables.slice(0, 260)}`);
    h.check(/A N 2 10\.50 0\.71/.test(tables) && /C S 2 22\.50 0\.71/.test(tables), 'each combination has its count, mean, and standard deviation');
    h.check(/Stat group A 4 11\.50/.test(tables) && /Stat site N 6 15\.50/.test(tables), 'and each value of each grouping has its mean whatever the other is');
    h.check(/Type III sums of squares/.test(tables), 'the table says which sums of squares');
    h.check((await lastPage().getByTestId('stats-chart').count()) >= 2, 'the cells are drawn as box plots and as means with intervals');
    h.check(/not which ones/.test(await text(lastPage().getByTestId('stats-reading'))), 'the reading says that the test does not say which means differ');
    await axe('Statistics, a two-way analysis of variance');

    await goal('scale_reliability');
    h.equal(await page.getByTestId('stats-run').isDisabled(), true, 'alpha cannot run before two items are chosen');
    await page.getByTestId('stats-list-0').selectOption({ label: 'Stat score (scale)' });
    await page.getByTestId('stats-list-add').click();
    await page.getByTestId('stats-list-1').selectOption({ label: 'Stat age (scale)' });
    await page.getByTestId('stats-list-add').click();
    await page.getByTestId('stats-list-2').selectOption({ label: 'Stat hours (scale)' });
    await page.getByTestId('stats-method-cronbach_alpha').check();
    await run();
    // R: alpha from cov() .7780, standardized .9168, mean correlation .786, Feldt's interval [.4125, .9306]; leaving out Stat age gives .954.
    const said = await sentenceOf();
    h.check(/^Cronbach's alpha for the 3 items was \.78, 95% CI \[\.41, \.93\], N = 12\.$/.test(said), `alpha as a report would write it: ${said}`);
    const alpha = await tablesOf();
    h.check(/\.778 \.917 \.786 3 12 \[\.41\d, \.93\d\]/.test(alpha), `the table of alpha: ${alpha.slice(0, 200)}`);
    h.check(/\.730 \.954/.test(alpha), `an item's correlation with the rest, and alpha without it: ${alpha.slice(0, 400)}`);
    h.check(/Leaving out Stat age would raise alpha to \.95\./.test(await text(lastPage().getByTestId('stats-reading'))), 'the reading names the item whose leaving out raises alpha');
    await axe("Statistics, Cronbach's alpha");

    // The whole analysis of alpha can be bootstrapped; the script repeats the resamples in R below, and R must print the same intervals. (The two-way analysis can be
    // bootstrapped too, but cells of two cases are often left empty by a resample, and the engine then refuses the bootstrap, so its R code is only parsed here.)
    await page.getByTestId('stats-bootstrap-on').click();
    await page.getByTestId('stats-resamples').selectOption('1000');
    await page.getByTestId('stats-seed').fill('5');
    await run();
    const wholeTable = (pg) => text(pg.locator('.table-wrap').filter({ hasText: 'Bootstrap of the whole analysis' }));
    const bootAlpha = await wholeTable(lastPage());
    h.check(/^Bootstrap of the whole analysis Statistic Estimate Bootstrap SE .* α \.78 /.test(bootAlpha) && /Standardized α/.test(bootAlpha) && /Mean correlation between items/.test(bootAlpha), `alpha is bootstrapped as a whole analysis: ${bootAlpha.slice(0, 260)}`);
    await page.getByTestId('stats-bootstrap-on').click();
  });

  await S('the other tests are run too, so that every kind of page has its R code run in R below', async () => {
    const sentence = async () => text(lastPage().getByTestId('stats-sentence'));
    await goal('against_value');
    await pick('x', 'Stat score (scale)');
    await page.getByTestId('stats-mu').fill('15');
    await page.getByTestId('stats-method-one_sample_t').check();
    await run();
    h.check(/Stat score .*15/.test(await sentence()), `a one-sample t test: ${await sentence()}`);
    await page.getByTestId('stats-method-wilcoxon').check();
    await run();
    await goal('compare_paired');
    await pick('first', 'Stat score (scale)');
    await pick('second', 'Stat age (scale)');
    await page.getByTestId('stats-method-paired_t').check();
    await run();
    await page.getByTestId('stats-method-wilcoxon').check();
    await run();
    await goal('compare_groups');
    await pick('outcome', 'Stat score (scale)');
    await pick('group', 'Stat group (nominal)');
    await page.getByTestId('stats-method-kruskal_wallis').check();
    await run();
    h.check((await text(lastPage().getByTestId('stats-title'))).includes('Kruskal-Wallis'), 'Kruskal-Wallis, with Dunn\'s pairs');
    await goal('correlate');
    await pick('x', 'Stat score (scale)');
    await pick('y', 'Stat age (scale)');
    await page.getByTestId('stats-method-kendall').check();
    await run();
    await goal('association');
    await pick('row', 'Stat site (nominal)');
    await pick('column', 'Stat later (nominal)');
    await page.getByTestId('stats-method-fisher').check();
    await run();
    h.check((await text(lastPage().getByTestId('stats-title'))).includes("Fisher's exact"), "Fisher's exact test");

    // A test on some of the cases only: the script picks them with the same rule.
    await goal('correlate');
    await pick('x', 'Stat score (scale)');
    await pick('y', 'Stat age (scale)');
    await page.getByTestId('stats-method-pearson').check();
    await page.getByTestId('stats-filter-rule').fill('`Stat age` >= 30 & `Stat site` == "N"');
    await run();
    h.check(/Only cases where/.test(await text(lastPage().getByTestId('stats-page-filter'))), 'a test on the cases a rule picks');
    await page.getByTestId('stats-filter-rule').fill('');
  });

  // ---- what leaves the page: a table to paste, a chart as a picture, and the whole notebook as a file

  await S('a table is copied as text for a spreadsheet and as a table for a document', async () => {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    const copy = page.locator('[data-procedure=one_way_anova]').first().getByTestId('stats-copy-table').first();
    h.check((await copy.getAttribute('aria-label')).startsWith('Copy the table "'), `each button names its table: ${await copy.getAttribute('aria-label')}`);
    await copy.click();
    // The clipboard is written after the click returns, so it is read until it holds the table.
    let board = {};
    for (let i = 0; i < 50 && !board['text/plain']; i++) {
      board = await page.evaluate(async () => {
        const out = {};
        try {
          for (const item of await navigator.clipboard.read()) for (const type of item.types) out[type] = await (await item.getType(type)).text();
        } catch {
          // nothing on the clipboard yet
        }
        return out;
      });
      if (!board['text/plain']) await page.waitForTimeout(100);
    }
    h.check(board['text/plain'].split('\n').length >= 3 && board['text/plain'].split('\n')[0].includes('\t'), `rows on lines and cells between tabs: ${JSON.stringify(board['text/plain'].slice(0, 120))}`);
    h.check(/<table><caption>.*<\/caption><thead><tr><th>/.test(board['text/html']) && board['text/html'].includes('<td>'), 'and the same table as HTML, which Word pastes as a table');
  });

  await S('a chart is saved as a picture, as PNG and as SVG, in light colors whatever the theme, and a file is never replaced', async () => {
    const figure = page.locator('[data-procedure=one_way_anova]').first().locator('figure.stats-chart').first();
    const viewWidth = await figure.locator('svg.stats-svg').evaluate((s) => s.viewBox.baseVal.width);
    const target = path.join(workDir, 'chart-picture');
    await page.emulateMedia({ colorScheme: 'dark' });
    await figure.getByTestId('stats-chart-save').click();
    const dlg = page.getByTestId('stat-export-dialog');
    await dlg.waitFor();
    h.check((await text(dlg.getByRole('heading'))).includes('Save the chart as a picture'), 'the window says what it is saving');
    h.check((await text(dlg)).includes('PNG') && (await text(dlg)).includes('SVG') && !(await text(dlg)).includes('Excel'), 'and offers the two kinds of picture only');
    await axe('Statistics, saving a chart as a picture');
    await dlg.getByTestId('stat-export-path').fill(target);
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*chart-picture\.png/).waitFor();
    const png = fs.readFileSync(target + '.png');
    h.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', 'a PNG file, with the ending added');
    h.equal(png.readUInt32BE(16), viewWidth * 2, `twice as many pixels across as the chart is wide (${viewWidth})`);
    // The picture is read back and its pixels counted: a white page with dark words, though the window is in its dark theme.
    const pixels = await page.evaluate(async (b64) => {
      const img = new Image();
      img.src = 'data:image/png;base64,' + b64;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext('2d');
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(0, 0, c.width, c.height).data;
      let dark = 0;
      let coloured = 0;
      for (let i = 0; i < d.length; i += 4) {
        const max = Math.max(d[i], d[i + 1], d[i + 2]);
        const min = Math.min(d[i], d[i + 1], d[i + 2]);
        if (max < 90) dark++;
        if (max - min > 60) coloured++;
      }
      return { corner: [d[0], d[1], d[2], d[3]], dark, coloured };
    }, png.toString('base64'));
    h.equal(pixels.corner, [255, 255, 255, 255], 'the picture has a white background in the dark theme');
    h.check(pixels.dark > 200 && pixels.coloured > 200, `with dark words and colored marks drawn on it: ${JSON.stringify(pixels)}`);

    await dlg.getByTestId('stat-export-svg').click();
    await dlg.getByTestId('stat-export-path').fill(target);
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*chart-picture\.svg/).waitFor();
    const svg = fs.readFileSync(target + '.svg', 'utf8');
    h.check(svg.startsWith('<svg') && svg.includes('xmlns="http://www.w3.org/2000/svg"') && svg.includes('<title>'), 'the SVG is a file of its own, with the name of the chart in it');
    h.check(!/class="|tabindex=|var\(--/.test(svg), 'and has no class, focus stop, or style variable that needs the page');
    h.check(/<rect[^>]*fill="#ffffff"/.test(svg), 'and a white background');
    const words = svg.match(/<text[^>]*style="([^"]*)"/);
    const fill = words && words[1].match(/fill:rgb\((\d+), (\d+), (\d+)\)/);
    h.check(fill && Number(fill[1]) < 120 && Number(fill[2]) < 120 && Number(fill[3]) < 120, `its words are dark, though the window is dark: ${words && words[1]}`);
    h.check(/font-family:'Segoe UI'/.test(svg), 'and name a font nearly everyone has');

    await dlg.getByTestId('stat-export-path').fill(target + '.svg');
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/already exists/).waitFor();
    h.equal(fs.readFileSync(target + '.svg', 'utf8'), svg, 'a file that is already there is not replaced');
    await dlg.getByTestId('stat-export-close').click();
    await dlg.waitFor({ state: 'detached' });
    await page.emulateMedia({ colorScheme: 'light' });
    keep(target + '.png', target + '.svg');
  });

  await S('the whole notebook is exported as Word, Excel, and a web page, with every page, its APA tables, and its charts', async () => {
    const pages = await page.getByTestId('stats-page').count();
    const tables = await page.getByTestId('stats-table').count();
    const charts = await page.locator('figure.stats-chart').count();
    const firstSentence = await text(page.getByTestId('stats-sentence').first());
    const target = path.join(workDir, 'notebook-check');
    await page.getByTestId('stats-export').click();
    const dlg = page.getByTestId('stat-export-dialog');
    await dlg.waitFor();
    h.check((await text(dlg.getByTestId('stat-export-count'))).includes(`${pages} pages are in the notebook`), `the window says how many pages it will export: ${(await text(dlg)).slice(0, 160)}`);
    h.check((await text(dlg)).includes('Word') && (await text(dlg)).includes('Excel') && (await text(dlg)).includes('Web page'), 'and offers Word, Excel, and a web page');
    await axe('Statistics, exporting the notebook');
    await dlg.getByTestId('stat-export-title').fill('Stats export check');
    await dlg.getByTestId('stat-export-path').fill(target);

    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*notebook-check\.docx/).waitFor({ timeout: 120000 });
    const doc = unzipEntry(target + '.docx', 'word/document.xml').toString('utf8');
    h.check(doc.includes('Stats export check'), 'the Word file has the title');
    h.equal((doc.match(/w:val="Heading2"/g) ?? []).length, pages, `a heading for each of the ${pages} pages`);
    h.equal((doc.match(/w:val="CaptionNumber"\/><\/w:pPr><w:r><w:rPr><w:b\/><\/w:rPr><w:t xml:space="preserve">Table \d+</g) ?? []).length, tables, `a numbered caption for each of the ${tables} tables`);
    h.equal((doc.match(/w:val="CaptionNumber"\/><\/w:pPr><w:r><w:rPr><w:b\/><\/w:rPr><w:t xml:space="preserve">Figure \d+</g) ?? []).length, charts, `and for each of the ${charts} charts`);
    h.check(!doc.includes('TableGrid') && !doc.includes('w:insideV'), 'the tables have no grid and no vertical lines');
    h.check(doc.includes(firstSentence.slice(0, 60)), `the results sentences are the page's own: ${firstSentence.slice(0, 60)}`);
    h.check(doc.includes('differed significantly between the 3 groups of Stat group'), 'including the analysis of variance');
    h.check(unzipEntry(target + '.docx', `word/media/image${charts}.png`) !== null && unzipEntry(target + '.docx', `word/media/image${charts + 1}.png`) === null, `a picture for each of the ${charts} charts`);
    h.check(/<wp:docPr id="1" name="[^"]*" descr="[^"]{20,}"/.test(doc), 'and each picture carries its text alternative');

    await dlg.getByTestId('stat-export-xlsx').click();
    h.equal(await dlg.getByTestId('stat-export-path').inputValue(), target, 'choosing another kind of file drops the ending of the one just saved');
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*notebook-check\.xlsx/).waitFor({ timeout: 120000 });
    h.check(unzipEntry(target + '.xlsx', `xl/worksheets/sheet${pages + 1}.xml`) !== null && unzipEntry(target + '.xlsx', `xl/worksheets/sheet${pages + 2}.xml`) === null, `Excel has a Contents sheet and a sheet for each of the ${pages} pages`);
    const book = unzipEntry(target + '.xlsx', 'xl/workbook.xml').toString('utf8');
    h.check(book.includes('name="Contents"') && book.includes('name="1 Page 1'), 'named for them');
    const sheet = unzipEntry(target + '.xlsx', 'xl/worksheets/sheet2.xml').toString('utf8');
    h.check(sheet.includes('<v>') && sheet.includes('t="inlineStr"'), 'with numbers as numbers and words as words');

    await dlg.getByTestId('stat-export-html').click();
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*notebook-check\.html/).waitFor({ timeout: 120000 });
    const html = fs.readFileSync(target + '.html', 'utf8');
    h.check(html.startsWith('<!doctype html>') && html.includes('<html lang="en">') && html.includes('<title>Stats export check</title>'), 'the web page is a document with a language and a title');
    h.equal((html.match(/<section id="page-\d+"/g) ?? []).length, pages, 'a section for each page');
    h.equal((html.match(/<img src="data:image\/svg\+xml;base64,/g) ?? []).length, charts, 'a drawing for each chart');
    h.check(!/alt=""/.test(html) && !html.includes('<script'), 'each with a text alternative, and nothing in the page runs');
    const exported = await page.context().newPage();
    try {
      await exported.goto('file:///' + (target + '.html').replace(/\\/g, '/'));
      const r = await new ctx.AxeBuilder({ page: exported }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa', 'best-practice']).analyze();
      h.check(r.violations.length === 0, `the exported page has no accessibility violation (axe): ${r.violations.map((v) => `${v.id}: ${v.help}`).join('; ')}`);
      h.equal(await exported.locator('img').evaluateAll((els) => els.filter((e) => !e.complete || e.naturalWidth === 0).length), 0, 'and every drawing in it shows');
      if (process.env.PRISMQDA_CHART_SHOTS) {
        await exported.setViewportSize({ width: 1000, height: 2600 });
        await exported.screenshot({ path: path.join(process.env.PRISMQDA_CHART_SHOTS, 'exported-notebook.png') });
      }
    } finally {
      await exported.close();
    }
    await dlg.getByTestId('stat-export-close').click();
    await dlg.waitFor({ state: 'detached' });
    keep(target + '.docx', target + '.xlsx', target + '.html');
  });

  await S('a weighted analysis is on the notebook, so the script that R runs below counts the cases the way the page does', async () => {
    // Stat times says how many times each of the twelve respondents is counted; Stat age is described with those weights.
    await rpc('variable.define', { name: 'Stat times', level: 'ratio' });
    const d = await rpc('data.get');
    const weights = { R1: 2, R2: 1, R3: 3, R4: 1, R5: 2, R6: 2, R7: 1, R8: 1, R9: 3, R10: 1, R11: 2, R12: 0 };
    await rpc('variable.set_values', { variable: 'Stat times', changes: d.cases.filter((c) => c.name in weights).map((c) => ({ case_id: c.id, value: String(weights[c.name]) })) });
    await rpc('stats.notebook.run', { request: { procedure: 'describe', variables: ['attr:Stat age'], options: { weight: 'Stat times' } }, section: '' });
    await page.reload();
    await open();
    const weighted = page.getByTestId('stats-page').filter({ has: page.getByTestId('stats-page-weight') });
    await weighted.first().waitFor();
    // Other steps of the whole check have cases with the same names, so the sums are the engine's own, which the page must repeat.
    const notebook = await rpc('stats.notebook', {});
    const mine = notebook.filter((p) => p.kind === 'analysis' && p.request?.options?.weight === 'Stat times').at(-1);
    const line = await text(weighted.last().getByTestId('stats-page-weight'));
    h.check(mine.result.weight.counted >= 19 && line.includes(`count as ${mine.result.weight.counted}.`) && line.includes('Stat times'), `the page says how many cases the weights come to (${mine.result.weight.counted}): ${line}`);
  });

  await S('a page run on some of the cases says so on the page, and the page is taken off the notebook again', async () => {
    const d = await rpc('data.get');
    const some = d.cases.filter((c) => /^R\d+$/.test(c.name)).slice(0, 5).map((c) => c.id);
    h.equal(some.length, 5, 'the check has five cases to choose');
    const added = await rpc('stats.notebook.run', { request: { procedure: 'describe', variables: ['attr:Stat age'], options: { case_ids: some } }, section: '' });
    await page.reload();
    await open();
    const mine = page.locator(`#stats-page-${added.id}`);
    await mine.waitFor();
    const line = await text(mine.getByTestId('stats-page-cases'));
    h.check(/^Run on 5 cases chosen when it was made \(the working set\), not on every case\./.test(line), `the page says it was run on five cases: ${line}`);
    h.equal(await page.getByTestId('stats-page-cases').count(), 1, 'and no other page says so');
    await axe('Statistics, a page run on some of the cases');
    await rpc('stats.notebook.remove', { id: added.id });
    await page.reload();
    await open();
    await page.getByTestId('stats-page').first().waitFor();
    h.equal(await page.getByTestId('stats-page-cases').count(), 0, 'the page is taken off again, so the R script below holds only the pages it was checked with');
  });

  await S('the notebook is exported as an R script, and R, run on it, prints the numbers each page shows', async () => {
    const pages = await page.getByTestId('stats-page').evaluateAll((els) =>
      els.map((el) => ({
        title: el.querySelector('[data-testid=stats-title]').innerText.replace(/\s+/g, ' ').trim(),
        // The tables of the page, the bootstrap intervals among them; text content, because a table in a closed part of the page has no inner text.
        cells: [...el.querySelectorAll('table[data-testid=stats-table]')].flatMap((tb) => [...tb.querySelectorAll('tbody td')]).map((c) => c.textContent.replace(/\s+/g, ' ').trim()),
        // The numbers behind the charts. The points of a scatter plot or of curves are not printed, nor are those of a normal probability plot of over a hundred points.
        chartCells: [...el.querySelectorAll('figure[data-testid=stats-chart]')]
          .filter((f) => !['scatter', 'curves'].includes(f.getAttribute('data-chart') ?? '') && !(f.getAttribute('data-chart') === 'qq' && f.querySelectorAll('details.stats-chart-table tbody tr').length > 100))
          .flatMap((f) => [...f.querySelectorAll('details.stats-chart-table tbody td')])
          .map((c) => c.textContent.replace(/\s+/g, ' ').trim()),
        charts: el.querySelectorAll('figure[data-testid=stats-chart]').length,
        code: el.querySelector('[data-testid=stats-script] code')?.textContent ?? '',
      })),
    );
    // The code of a page is a part of the page, read like the rest of it.
    const first = page.getByTestId('stats-script').first();
    await first.locator('summary').click();
    h.check((await text(first)).includes('The R code for this page') && (await first.locator('pre code').innerText()).includes('d <- data'), 'a page shows its R code when it is asked for');
    await axe('Statistics, the R code of a page');
    await first.locator('summary').click();
    const withCode = pages.filter((x) => x.code.length > 0).length;
    h.equal(withCode, pages.length, 'every page has its R code on it');
    const target = path.join(workDir, 'notebook-script');
    await page.getByTestId('stats-export').click();
    const dlg = page.getByTestId('stat-export-dialog');
    await dlg.waitFor();
    await dlg.getByTestId('stat-export-r').click();
    await dlg.getByTestId('stat-export-title').fill('Stats script check');
    await dlg.getByTestId('stat-export-path').fill(target);
    await dlg.getByTestId('stat-export-save').click();
    await dlg.getByText(/Saved .*notebook-script\.R and its data file .*notebook-script-data\.csv/).waitFor({ timeout: 60000 });
    await dlg.getByTestId('stat-export-close').click();
    await dlg.waitFor({ state: 'detached' });
    const script = fs.readFileSync(target + '.R', 'utf8');
    const csv = fs.readFileSync(target + '-data.csv', 'utf8');
    keep(target + '.R', target + '-data.csv');
    h.check(script.includes('read.csv("notebook-script-data.csv"') && !script.includes('{{data-file}}'), 'the script reads the data file it was saved with');
    const chartCount = pages.reduce((n, x) => n + x.charts, 0);
    h.check(chartCount > 0 && script.includes('"-charts.pdf"') && script.includes('invisible(dev.off())'), `the script draws the ${chartCount} charts of the notebook into a PDF beside the data file`);
    h.check(/pq_boot\(/.test(script) && /pq_boot_whole\(/.test(script) && script.includes('pq_draws <- function'), 'it repeats the bootstrap of a statistic and of a whole analysis, with the generator written out');
    h.equal((script.match(/^# ---- Page \d+:/gm) ?? []).length, pages.length, 'and has a section for every page');
    const caseCount = Number((await text(page.getByTestId('stats-case-count'))).match(/(\d+) cases?/)?.[1]);
    h.check(csv.split('\n')[0].includes('Stat score') && csv.trim().split('\n').length === caseCount + 1, `the data has a row for each of the ${caseCount} cases: ${csv.split('\n')[0]}`);

    const rscript = [process.env.PRISMQDA_RSCRIPT, 'C:\\Program Files\\R\\R-4.5.3\\bin\\Rscript.exe'].find((f) => f && fs.existsSync(f));
    if (!rscript) {
      h.check(true, 'R was not found on this computer, so the script was not run (set PRISMQDA_RSCRIPT to its Rscript.exe)');
      return;
    }
    const ran = spawnSync(rscript, [target + '.R'], { cwd: workDir, encoding: 'utf8', timeout: 900000, maxBuffer: 256 * 1024 * 1024 });
    fs.writeFileSync(path.join(workDir, 'script-output.txt'), `${ran.stdout}\n--- stderr ---\n${ran.stderr}`);
    keep(path.join(workDir, 'script-output.txt'));
    h.equal(ran.status, 0, `R ran the whole script without stopping: ${(ran.stderr ?? '').split('\n').filter((l) => /Error|error/.test(l)).slice(0, 3).join(' | ')}`);
    const pdfFile = target + '-charts.pdf';
    h.check(fs.existsSync(pdfFile) && fs.statSync(pdfFile).size > 2000, `R drew the charts into ${path.basename(pdfFile)}`);
    if (fs.existsSync(pdfFile)) keep(pdfFile);

    // Each page's section of what R printed, and the numbers in it.
    const out = ran.stdout ?? '';
    const sections = new Map();
    let current = null;
    for (const line of out.split('\n')) {
      const m = line.match(/^Page (\d+): /);
      if (m) {
        current = Number(m[1]);
        sections.set(current, []);
      } else if (current !== null) sections.get(current).push(line);
    }
    const numbersIn = (s) => (s.replace(/\u2212/g, '-').match(/-?\d*\.?\d+(?:e[+-]?\d+)?/gi) ?? []).map(Number).filter(Number.isFinite);
    const report = [];
    let weak = 0;
    pages.forEach((pg, i) => {
      const r = numbersIn(sections.get(i + 1)?.join(' ') ?? '');
      const wanted = [];
      for (const cell of pg.cells) {
        if (/[<>]/.test(cell)) continue;
        const clean = cell.replace(/\u2212/g, '-').replace(/(\d),(?=\d{3})/g, '$1');
        for (const m of clean.match(/-?\d*\.\d+|-?\d+/g) ?? []) wanted.push(m);
      }
      for (const cell of pg.chartCells) {
        if (/[<>]/.test(cell)) continue;
        const clean = cell.replace(/\u2212/g, '-').replace(/(\d),(?=\d{3})/g, '$1');
        for (const m of clean.match(/-?\d*\.\d+|-?\d+/g) ?? []) wanted.push(m);
      }
      const missed = wanted.filter((tok) => {
        const decimals = (tok.split('.')[1] ?? '').length;
        // A number of over a hundred thousand is shown in full, and R's own working may differ from the engine's in its last digits.
        const tolerance = Math.max(0.5 * 10 ** -decimals + 1e-9, Math.abs(Number(tok)) * 1e-6);
        const x = Number(tok);
        return !r.some((y) => Math.abs(y - x) <= tolerance || Math.abs(Math.abs(y) - Math.abs(x)) <= tolerance);
      });
      const share = wanted.length === 0 ? 1 : 1 - missed.length / wanted.length;
      report.push(`${pg.title.slice(0, 60)}: ${wanted.length - missed.length}/${wanted.length}${missed.length ? ` missing ${missed.slice(0, 8).join(' ')}` : ''}`);
      if (missed.length > 0 && share < 1) weak++;
    });
    fs.writeFileSync(path.join(workDir, 'script-comparison.txt'), report.join('\n'));
    keep(path.join(workDir, 'script-comparison.txt'));
    h.equal(weak, 0, `on every page R prints every number in the tables of the page, the bootstrap intervals and the numbers behind the charts among them: ${report.filter((x) => /missing/.test(x)).slice(0, 12).join(' || ')}`);
  });

  await S('the notebook is kept in the project: a note on a page, a section, and a page of notes are all there when the page is opened again', async () => {
    const saved = async (test, what) => {
      for (let i = 0; i < 40; i++) {
        if (test(await rpc('stats.notebook', {}))) return true;
        await new Promise((r) => setTimeout(r, 150));
      }
      h.check(false, `${what} was kept in the project`);
      return false;
    };
    const before = await page.getByTestId('stats-page').count();
    h.check(before > 10, `the notebook holds the pages of every step so far: ${before}`);
    const first = page.getByTestId('stats-page').first();
    await first.getByTestId('stats-note').fill('Look at this again beside the interviews.');
    await first.getByTestId('stats-note').press('Tab');
    await first.getByTestId('stats-section-input').fill('Early look');
    await first.getByTestId('stats-section-input').press('Enter');
    await saved((pages) => pages[0]?.note === 'Look at this again beside the interviews.' && pages[0]?.section === 'Early look', 'the note and the section');
    h.check(true, 'a note and a section are kept in the project');

    await page.getByTestId('stats-add-note').click();
    const notes = page.getByTestId('stats-note-page');
    await notes.waitFor();
    await notes.getByTestId('stats-note-title').fill('What to ask next');
    await notes.getByTestId('stats-note-title').press('Enter');
    await notes.getByTestId('stats-note-body').fill('Check the second site.');
    await notes.getByTestId('stats-note-body').press('Tab');
    await saved((pages) => pages.some((x) => x.kind === 'note' && x.title === 'What to ask next' && x.note === 'Check the second site.'), 'the page of notes');
    h.equal(await page.getByTestId('stats-page').count(), before, 'a page of notes is not an analysis, and does not change the numbering');
    await axe('Statistics, the notebook with a page of notes');

    // The index lists every page, and a section is a tab.
    h.equal(await page.getByTestId('stats-index-item').count(), before + 1, 'the index has a line for each page, notes included');
    const tabs = await text(page.getByTestId('stats-sections'));
    h.check(tabs.includes('All pages') && tabs.includes('Early look') && tabs.includes('No section'), `the sections are tabs: ${tabs}`);
    await page.getByTestId('stats-section-Early look').click();
    h.equal(await page.getByTestId('stats-page').count(), 1, 'one section shows only its own pages');
    h.equal(await page.getByTestId('stats-note-page').count(), 0, 'and not the page of notes, which is in no section');
    await page.getByTestId('stats-section-all').click();
    h.equal(await page.getByTestId('stats-page').count(), before, 'all pages again');

    // A page moves.
    await notes.getByTestId('stats-move-up').click();
    await saved((pages) => pages.findIndex((x) => x.kind === 'note') === pages.length - 2, 'the order');
    await notes.getByTestId('stats-move-down').click();
    await saved((pages) => pages[pages.length - 1].kind === 'note', 'the order, put back');

    // Close and open the page again: the notebook is read from the project.
    const reopened = Date.now();
    await page.reload();
    await open();
    await page.getByTestId('stats-page').first().waitFor();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length === n, before, { timeout: 60000 });
    console.log(`       (the notebook of ${before} pages came back in ${Date.now() - reopened} ms)`);
    h.check(Date.now() - reopened < 30000, `the notebook of ${before} pages is back in under 30 seconds`);
    h.equal(await page.getByTestId('stats-page').count(), before, 'every analysis is there after the page is opened again');
    h.equal(await page.getByTestId('stats-note').first().inputValue(), 'Look at this again beside the interviews.', 'with the note on it');
    h.equal(await page.getByTestId('stats-note-title').inputValue(), 'What to ask next', 'and the page of notes');
    h.check((await text(page.getByTestId('stats-title').first())).startsWith('Page 1:'), 'the pages are numbered as before');
  });

  await S('running the analyses again after the data changed keeps what each page said and shows the old and the new side by side', async () => {
    const table = await rpc('stats.table', { keys: ['attr:Stat score'] });
    const at = table.columns[0].values.findIndex((v) => v !== null);
    const original = table.columns[0].values[at];
    const caseId = table.cases[at].id;
    await page.getByTestId('stats-rerun-all').click();
    await page.getByTestId('stats-rerun-summary').waitFor({ timeout: 120000 });
    h.check((await text(page.getByTestId('stats-rerun-summary'))).includes('None of them changed'), `with the data as it was, nothing changes: ${await text(page.getByTestId('stats-rerun-summary'))}`);

    await rpc('variable.set_values', { variable: 'Stat score', changes: [{ case_id: caseId, value: '90' }] });
    await page.reload();
    await open();
    await page.getByTestId('stats-page').first().waitFor();
    await page.getByTestId('stats-rerun-all').click();
    await page.waitForFunction(() => /changed; /.test(document.querySelector('[data-testid=stats-rerun-summary]')?.textContent ?? ''), null, { timeout: 120000 });
    const summary = await text(page.getByTestId('stats-rerun-summary'));
    h.check(/analyses were run again/.test(summary) && !/None of them changed/.test(summary), `some pages changed: ${summary}`);
    const compares = await page.getByTestId('stats-compare').count();
    h.check(compares > 3, `each page that changed shows what it said before: ${compares}`);
    const same = await page.getByTestId('stats-rerun-same').count();
    h.check(same > 0, `and a page that did not use the score says that nothing changed: ${same}`);
    const one = await text(page.getByTestId('stats-compare').first());
    h.check(one.includes('Earlier run') && one.includes('This run') && one.includes('Change') && one.includes('Cases used'), `the old and the new side by side: ${one.slice(0, 220)}`);
    await axe('Statistics, the analyses run again');

    // What a page said is kept: after the page is opened again, it can be compared with the run before.
    await page.reload();
    await open();
    await page.getByTestId('stats-page').first().waitFor();
    h.check((await page.getByTestId('stats-compare-load').count()) > 3, 'a page that changed offers the comparison again after the project is opened again');
    await page.getByTestId('stats-compare-load').first().click();
    await page.getByTestId('stats-compare').first().waitFor();
    // The same analysis can be run on its own.
    await page.getByTestId('stats-rerun').first().click();
    await page.waitForFunction(() => document.querySelectorAll('[data-testid=stats-rerun-same], [data-testid=stats-compare]').length > 0);

    // Put the data back, and run again, so the pages say what the project holds.
    await rpc('variable.set_values', { variable: 'Stat score', changes: [{ case_id: caseId, value: String(original) }] });
    await page.getByTestId('stats-rerun-all').click();
    await page.waitForFunction(() => /changed; /.test(document.querySelector('[data-testid=stats-rerun-summary]')?.textContent ?? ''), null, { timeout: 120000 });
  });

  await S('a page can be removed from the notebook, and the notebook survives going to another page', async () => {
    const before = await page.getByTestId('stats-page').count();
    await lastPage().getByRole('button', { name: /^Remove page/ }).click();
    await page.waitForFunction((n) => document.querySelectorAll('[data-testid=stats-page]').length === n, before - 1, { timeout: 10000 }).catch(() => undefined);
    h.equal(await page.getByTestId('stats-page').count(), before - 1, 'one page fewer');
    h.equal((await rpc('stats.notebook', {})).filter((x) => x.kind === 'analysis').length, before - 1, 'and it is gone from the project too');
    await page.getByRole('tab', { name: 'Home', exact: true }).click();
    await page.getByRole('tab', { name: 'Statistics', exact: true }).click();
    await page.getByRole('button', { name: 'Statistics page', exact: true }).click();
    await page.getByRole('heading', { level: 2, name: 'Statistics', exact: true }).waitFor();
    h.equal(await page.getByTestId('stats-page').count(), before - 1, 'the notebook is still there after leaving the page');
    await axe('Statistics, a notebook of several pages');
  });
}
