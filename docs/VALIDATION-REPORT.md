# Validation report

What has been checked against an outside reference, how, and what has not. Written 2026-11-07, extended the same day for the statistics procedures.

**In one sentence:** the agreement coefficients and the statistics procedures that PrismQDA calculates (the probability distributions, descriptives, t tests, one-way analysis of variance, chi-square and Fisher's tests, correlations, the rank-based tests, the tests of counts and distributions, the measures of order, and partial correlation) match R to the precision each reference can give, on invented data that cover the shapes real data take, including ties, zero differences, empty corners, and very large degrees of freedom.

## What is validated

### Agreement coefficients (`crates/prismqda-stats`)

| Coefficient | Reference | Compared to |
| --- | --- | --- |
| Percent agreement | R, written out from the definition; `irr::agree`; `irrCAC::pa.coeff.raw` | 1e-12, 1e-9, 6e-6 |
| Cohen's kappa (two coders) | `irr::kappa2` | 1e-9 |
| Scott's pi | R, written out from the definition; `irrCAC::fleiss.kappa.raw` (which equals Scott's pi for two coders) | 1e-12, 6e-6 |
| Fleiss' kappa | `irr::kappam.fleiss` (and the number of units it used) | 1e-9 |
| Brennan and Prediger's coefficient | R, written out; `irrCAC::bp.coeff.raw` | 1e-12, 6e-6 |
| Gwet's AC1 | R, written out; `irrCAC::gwet.ac1.raw` | 1e-12, 6e-6 |
| Krippendorff's alpha (nominal) | R, written out from the definition; `irr::kripp.alpha`; `irrCAC::krippen.alpha.raw` | 1e-12, 1e-9 (see below), 6e-6 |

How:

- The reference file is `crates/prismqda-stats/tests/reference/agreement_reference.json`. It is made by `make_agreement_reference.R` beside it, with **R 4.5.3, `irr` 0.85, `irrCAC` 1.4, and `jsonlite`**. The script writes the numbers with full precision, and the Rust test (`tests/agreement_reference.rs`) reads them and fails if any PrismQDA value is further from R's than the tolerance above. The `irrCAC` package rounds what it returns to five decimals, which is why it is compared more loosely.
- The data are **invented**. Nothing comes from a real study.
- Seven tables, chosen to cover what real coding produces: two coders and five codes (`two_coders_five_codes`, 40 units); the "kappa paradox", where agreement is high and almost everything is in one category (`kappa_paradox`, 100 units); three coders who all rated everything (`three_coders_complete`, 60); four coders with gaps (`four_coders_with_gaps`, 80); three coders where some units were rated by one coder or none (`three_coders_singletons`, 50); perfect agreement (`perfect_agreement`, 30); and chance-level agreement (`chance_level`, 90).
- More than sixty individual values are compared in one run. Further tests check the conventions that matter: units that no one or only one coder rated are ignored and not miscounted; Fleiss' kappa uses only the units every coder rated; and the kappa paradox shows up the way the literature says it does.

**One known disagreement with a package, recorded and pinned.** On `three_coders_complete`, `irr::kripp.alpha` 0.85 returns 0.300398, while the definition worked in exact fractions, `irrCAC`, and PrismQDA all give 0.302347. PrismQDA follows the definition. A test pins this exactly: if a newer `irr` changes its value, or a second table starts to differ, the test fails and someone looks.

### Probability distributions (`crates/prismqda-stats/src/dist.rs`)

Every p-value and interval below stands on these, so they are compared first, to R's own functions, over a grid that includes far tails, tiny and very large degrees of freedom, and values near the middle (`tests/distribution_reference.rs`, reference made by `make_distribution_reference.R` with base R 4.5.3).

| Function | R function | Compared to (relative error) |
| --- | --- | --- |
| Log gamma | `lgamma` | 1e-13 |
| Normal, both tails | `pnorm` | 1e-12 |
| Normal quantile | `qnorm` | 1e-12 |
| Student's t, lower tail and the two-sided tail | `pt` | 1e-11 |
| t quantile | `qt` | 1e-10 |
| Chi-square, both tails | `pchisq` | 1e-11 |
| Chi-square quantile | `qchisq` | 1e-10 |
| F, both tails | `pf` | 1e-11 |
| F quantile | `qf` | 1e-10 (for p from 0.025; see below) |

A p-value is computed from the tail it describes and never as one minus the other tail, so a p-value of 1e-20 keeps its digits. **Where R is the less accurate one:** R's `qf` returns 0 for `qf(1e-10, 1, 5)` (it computes `1 / qbeta(p, ...) - 1`, which loses every digit for a small p; the true value is about 1e-20), so the F quantile is compared only from p = 0.025 up. The reference file holds numbers with 15 significant digits, which is the most it can ask of a comparison.

### Statistics procedures (`crates/prismqda-stats`)

`tests/procedures_reference.rs` compares each result with the reference made by `make_procedures_reference.R` (base R 4.5.3; only `jsonlite` is used, to write the file). Where an R function exists it is the reference; where R has none (an effect size, a post-hoc test) the formula is written out in plain R from its definition and the script says so.

| Procedure | Reference | Compared to |
| --- | --- | --- |
| Descriptives: n, sum, mean, variance, SD, SE, interval of the mean, quartiles and median (R's default percentile method), skewness and kurtosis with their standard errors, count of outliers (1.5 times the interquartile range) | `mean`, `var`, `sd`, `t.test`, `quantile(type = 7)`; the skewness (G1) and excess kurtosis (G2) formulas written out | 1e-11 |
| One-sample, paired, Student, and Welch t tests: t, df, p, estimate, standard error, interval | `t.test` | 1e-11 |
| Cohen's d and Hedges' g | written out (Hedges and Olkin, 1985) | 1e-11 |
| Exact intervals of Cohen's d and Hedges' g (the noncentral t; Steiger and Fouladi, 1997): one sample, pairs, and two groups, at 90, 95, and 99 percent | the limits of the noncentrality parameter found with `uniroot(pt(t, df, ncp))` to 1e-12, times the scale (1 over the square root of n, or the square root of 1/n1 + 1/n2) | 1e-8 |
| One-way ANOVA: sums of squares, F, p; eta squared and omega squared | `anova(lm())`; the two effect sizes written out | 1e-11 |
| Welch's ANOVA | `oneway.test` | 1e-11 |
| Levene's test (deviations from the median) | `anova(lm())` of the absolute deviations | 1e-11 |
| Pairwise comparisons on the pooled variance: raw, Bonferroni, Holm, and Benjamini-Hochberg p-values; differences, standard errors, intervals (Bonferroni-widened if chosen) | `pairwise.t.test(pool.sd = TRUE)`; the differences and intervals written out | 1e-11 |
| Chi-square test: statistic, df, p, expected counts, standardized and adjusted residuals; Yates' correction for two by two | `chisq.test` | 1e-11 |
| G squared, phi, Cramer's V, contingency coefficient, smallest expected count, share of cells under 5 | written out | 1e-11 |
| Fisher's exact test (two by two): the two-sided p-value | `fisher.test` | 1e-10 |
| Fisher's: conditional odds ratio and its interval | the same calculation as `fisher.test` with the roots found to 1e-14; and `fisher.test` itself | 1e-8; and 3e-2 (see below) |
| Pearson correlation: r, t, df, p, Fisher-z interval | `cor.test` | 1e-11 |
| Spearman's rho and its p-value: exact for up to 9 pairs, the series approximation of AS 89 for 10 to 1,290 pairs, the t approximation when there are ties (the conditions of `cor.test`) | `cor.test(method = "spearman")`; and `exact = TRUE` on 13 samples of 5 to 100 pairs | 1e-11 for rho, 1e-6 for p |
| Kendall's tau-b: exact p (under 50 pairs, no ties) and the tie-adjusted normal approximation | `cor.test(method = "kendall")` | 1e-11 for tau and z, 1e-10 for p |
| Mann-Whitney U, mean ranks, exact and approximate p; rank-biserial correlation | `wilcox.test`, `rank`; the correlation written out | 1e-11, p 1e-10 |
| Wilcoxon signed-rank V, zero differences set aside, exact and approximate p; matched-pairs rank-biserial correlation | `wilcox.test` (paired and one sample) | 1e-11, p 1e-10 |
| Kruskal-Wallis H, df, p, epsilon squared, eta squared | `kruskal.test`; the two effect sizes written out | 1e-11 |
| Dunn's pairwise comparisons, with the correction for ties | written out in R (no package needed); adjusted with `p.adjust` | 1e-11 |
| p-value adjustment: Bonferroni, Holm, Benjamini-Hochberg | `p.adjust` | 1e-13 |

The exact Mann-Whitney, Wilcoxon, and Kendall p-values are used under exactly the conditions R uses them (both samples under 50 values and no ties, with no zero differences for the signed-rank test); each of those paths and each of the approximate paths is in the reference.

**Two places where R's own answer is the looser one, recorded.** (1) `fisher.test` finds the odds ratio and its interval with `uniroot` at its default tolerance (about 1.2e-4), which leaves a large odds ratio or interval end good to only a few thousandths to a percent: on the table 3, 1, 1, 3 the upper end is 621.93 in R and 626.24 when the roots are found to 1e-14 (0.69 percent apart), and on the table 1, 9, 11, 3 the lower end differs by 1.2 percent. PrismQDA solves to full precision, and the test compares it to a full-precision run of R's own method, so it is the more accurate of the two. (2) `qf` at tiny p, above.

**What these procedures do not do yet, said plainly.** Fisher's exact test is for two by two tables only. All tests here are two-sided; one-sided tests are not offered.

### Post-hoc comparisons, the Shapiro-Wilk test, and the noncentral t (`studentized.rs`, `anova.rs`, `normality.rs`, `noncentral.rs`)

`tests/posthoc_reference.rs` compares these with `reference/posthoc_reference.json`, made by `make_posthoc_reference.R` (base R 4.5.3; only `jsonlite` is used, to write the file). Games-Howell and Scheffe are not in base R, so the script writes them from their published formulas using R's `ptukey`, `qtukey`, `pt`, and `pf`; Tukey's test is R's own `TukeyHSD`.

| What | Reference | Compared to |
| --- | --- | --- |
| The studentized range distribution, `ptukey(q, k, df)`: 6 values of k from 2 to 12, 10 degrees of freedom from 2 to 40,000, 10 values of q | `ptukey` | 1e-9 relative |
| The quantiles `qtukey(p, k, df)`: PrismQDA's loop of the kind R uses (stopping when two steps differ by under 1e-4), and the full-accuracy root | `qtukey`; and the root of R's `ptukey(q) = p` found with `uniroot` to 1e-13 | 1e-9; and 2e-4 of the value against R's printed `qtukey` |
| Tukey's honestly significant difference (Tukey-Kramer when the groups differ in size): p-values and intervals, on 5 invented sets of 2 to 5 groups of unequal sizes and spreads (25 pairs) | `TukeyHSD`; the interval from the full-accuracy quantile | p: 1e-8 relative plus 1e-12 absolute; interval 1e-8; and R's printed interval within 2e-3 |
| Games-Howell: t, Welch degrees of freedom, p, interval | written out in R with `ptukey` and `qtukey` | the same |
| Scheffe: F, p, interval | written out in R with `pf` and `qf` | 1e-9 |
| The noncentral t, `pt(q, df, ncp)`: 5 degrees of freedom, 6 noncentralities, 6 values of q | `pt` | 1e-8 relative where the probability is over 1e-6; 1e-11 absolute elsewhere |
| Shapiro-Wilk W and p: 18 samples of 3 to 2,000 values (normal, exponential, uniform, lognormal, t, a sample with ties, and a regular sequence) | `shapiro.test` | W 1e-9, p 1e-6 |

**Where R's own answer is the looser one.** `qtukey` stops when two steps of its search differ by less than 1e-4, so its quantile, and the interval limits `TukeyHSD` prints, are good to about four digits. PrismQDA keeps going and solves for the quantile to full accuracy, so its Tukey and Games-Howell intervals can differ from R's printed ones in the fourth decimal of the quantile (a few ten-thousandths of the interval's half-width). The test compares PrismQDA with the root of R's own `ptukey`, at 1e-8. **Where nothing is tighter than R.** The p-value of a studentized range is found as 1 minus a probability near 1, so it has an absolute error of about 1e-15 and no more digits than that, in R as in PrismQDA; p-values under 1e-9 are compared with an absolute tolerance of 1e-12. Shapiro-Wilk is Royston's algorithm AS R94, as in R, which is itself an approximation of W's distribution (good to about the third decimal of p).

### Tests of counts, ranks, and distributions; measures of order; partial correlation (`discrete.rs`, `kolmogorov.rs`, `nonparametric.rs`, `contingency.rs`, `correlation.rs`)

`tests/nonparametric_reference.rs` and `tests/ordinal_reference.rs` compare these with `reference/nonparametric_reference.json` and `reference/ordinal_reference.json`, made by `make_nonparametric_reference.R` and `make_ordinal_reference.R` (base R 4.5.3; only `jsonlite` is used, to write the files). Where base R has the test it is the reference. Where it does not, the script writes the quantity out from its definition, and the check says how the written-out version was itself checked.

| What | Reference | Compared to |
| --- | --- | --- |
| The binomial test: p-value, estimate, and the Clopper-Pearson interval; 10 combinations of successes and cases (0 of 5 up to 250 of 400, including all successes and none) and of the share tested (0.1 to 0.8), each at three confidence levels (0.90, 0.95, 0.99) | `binom.test` | p 1e-8 relative; limits 1e-9 |
| The sign test (the binomial test of the signs at one half, zeros set aside): positives, negatives, p-value, median; 6 samples of 5 to 80 values, some with zeros, one nearly all of one sign, and one that is mostly zeros | `binom.test` on the signs; `median` | p 1e-8; median 1e-12 |
| McNemar's test: chi-square with and without the continuity correction, their p-values, and the exact p-value, 7 tables | `mcnemar.test`; `binom.test` for the exact p | chi-square 1e-12; p 1e-10; exact p 1e-8 |
| The one-sample chi-square (goodness of fit): statistic, degrees of freedom, p-value, expected counts, residuals; 5 sets of 2 to 5 categories (equal shares and named shares, one with a count of zero) | `chisq.test` | 1e-9 to 1e-12 |
| Friedman's test: chi-square corrected for ties, degrees of freedom, p-value, Kendall's W, mean ranks; 6 sets of 2 to 6 measures on 6 to 15 cases (some with many ties) | `friedman.test`; W from the statistic | 1e-9 to 1e-12 |
| Mood's median test: the pooled median, counts above and at or below it, chi-square, p-value, and Fisher's exact p for two groups; 4 sets of 2 to 4 groups | the counts written out in R, then `chisq.test(correct = FALSE)` and `fisher.test` | 1e-9 to 1e-12 |
| The Kolmogorov-Smirnov test against a normal curve: D, the exact p-value (Marsaglia, Tsang, and Wang) for fewer than 100 values, and the limiting p-value above that; 6 samples of 8 to 150 values | `ks.test` (exact and not) | D 1e-12; exact p 1e-8; limiting p 5e-5 (see below) |
| The two-sample Kolmogorov-Smirnov test: D and the exact p-value (ties handled as R does) while the two samples together have fewer than 10,000 pairs, and the limiting p-value above that; 6 pairs of samples of 6 to 120 values, two with ties | `ks.test` (exact and not) | D 1e-12; exact p 1e-8; limiting p 5e-5 |
| The runs test: runs, expected runs, standard deviation, z, the normal p-value with a continuity correction, and the exact two-sided p-value (twice the smaller tail of the exact distribution of the number of runs); 6 sequences of 10 to 60 values | written out in R from the definition and the exact distribution | 1e-9 to 1e-12 |
| The Moses test of extreme reactions: the exact one-sided p-value with none trimmed and with 5 percent of the control cases trimmed from each end (at least one); 3 pairs of samples of 3 to 11 values | written out in R by listing every arrangement of the ranks | 1e-9 |
| Partial correlation: r, t, p-value, and the Fisher interval; 3 sets of 12 to 40 cases with one or two controls held constant | the correlation of the `lm` residuals, which the script also checks against the inverse of the correlation matrix | r and t 1e-9; p 1e-8; limits 1e-9 |
| Gamma, Kendall's tau-b, Stuart's tau-c, Somers' d (both ways and symmetric), their standard errors, z, and p-values, 9 tables (2 by 2 to 5 by 3, strong, weak, negative, with a sparse cell) | the pairs counted one by one in R; tau-b is also `cor(method = "kendall")`; the standard errors are the large-sample formulas written out in R | 1e-10 |

**Where R's own answer is the looser one.** R sums the limiting Kolmogorov distribution only until a term is under 1e-6, and for a statistic under 1 it keeps only the first term, which leaves an error of up to about 1.5e-5 in the p-value near a statistic of 1. PrismQDA sums the series in full, so it is the more accurate of the two, and the check allows R's error (5e-5). **What is not in base R.** The runs test, the median test, the Moses test, partial correlation, and the standard errors of the measures of order are not in base R, so the reference script writes them out, and a script that is itself new code has to be checked. The standard errors were each checked in the script against the spread of the measure over 4,000 tables drawn from the same proportions (within 8 percent, which is the sampling error of that spread), and for a two by two table against Yule's closed form, which gives gamma's standard error exactly. The exact p-value of the Moses test is a count of every arrangement of the ranks. **Where the answer is an approximation.** The p-value of a one-sample Kolmogorov-Smirnov test whose curve was taken from the sample is too large (the test needs another table when the mean and the standard deviation are estimated), and the page says so beside the result and points to the Shapiro-Wilk test. The test is correct when the curve was fixed beforehand.

### Two-way analysis of variance and Cronbach's alpha (`factorial.rs`, `scale.rs`)

`tests/factorial_reference.rs` and `tests/scale_reference.rs` compare these with `reference/factorial_reference.json` and `reference/scale_reference.json`, made by `make_factorial_reference.R` and `make_scale_reference.R` (base R 4.5.3; only `jsonlite` is used, to write the files). The data are invented.

| What | Reference | Compared to |
| --- | --- | --- |
| The two-way analysis of variance, **type III** sums of squares with effects coded to add to zero: the sum of squares, degrees of freedom, F, p-value, and partial eta squared of each of the two variables and their interaction; the error sum of squares, degrees of freedom, and mean square; the total sum of squares; R squared and adjusted R squared; 5 designs (a balanced 2 by 3 with 6 cases a cell, an unbalanced 3 by 2, a 2 by 2 with a crossover interaction, an unbalanced 3 by 3 with cells of 3 to 8, and a 2 by 2 of cells of 2 and 3) | `drop1(lm(y ~ A * B, contrasts = contr.sum), scope = ~ A + B + A:B, test = "F")`, in which each effect is dropped from the full model while the others stay; `summary(lm)` for R squared; partial eta squared written out | 1e-9 relative |
| The counts, means, and standard deviations of each combination, the counts and means of each value of each variable, and the residual of each case | `tapply`; `resid(lm)` | 1e-9 |
| Levene's test of equal variances across the combinations (about the median): F, both degrees of freedom, and p-value | written out in R: the absolute deviation from the cell median, then `anova(lm(deviation ~ cell))` | 1e-9 |
| Cronbach's alpha: alpha, the standardized alpha, the mean correlation between items, the number of negative pairs, the mean and standard deviation of the sum of the items, and for each item its mean, standard deviation, correlation with the sum of the others, and alpha without it; 5 scales (5 items and 40 cases, 2 items, 4 items with one scored the other way, 8 items on 12 cases, and 3 weak items on 60 cases) | alpha written out from `cov()`, **and** in Hoyt's form `1 - MS(residual) / MS(cases)` of `anova(lm(score ~ case + item))`, which agree to the last digit; the others written out from `cor()` | 1e-11 relative |
| Feldt's interval for alpha (95 percent, and 90 percent for one scale) | written out: `1 - (1 - alpha) * qf(1 - a/2, n - 1, (n - 1)(k - 1))` and with `qf(a/2, ...)` | 1e-11 |

**What this does not check.** Type III sums of squares with effects that add to zero are the default of the statistical packages and what the page says it gives; a design with an empty combination is refused (the interaction could not be estimated), and the sums of squares of R's own `aov` are the sequential (type I) ones, which agree with these only for a balanced design. R's `car` and `psych` packages were not used (they are not installed), so the type III table is R's `drop1` and alpha is checked two ways in base R. The interval of alpha is Feldt's, which is not the one `psych::alpha` prints by default.

### The recorded script (`app/ui/src/lib/rcode.code.ts`, run by the interface check)

A page's R code is checked by running it. The interface check (`tools/scenarios-stats.mjs`) runs every kind of test on twelve invented respondents (28 kinds and variants, 57 pages, including a test on the cases a rule picks and pages with the bootstrap of a statistic and of a whole analysis), exports the notebook as an R script, runs the script in R 4.5.3 (`Rscript`), and reads what R printed for each page. For every number in the body of every table of every page (the bootstrap tables included, since the script writes out the generator the program draws its resamples with, and so are the numbers behind each chart, which the script prints as the page's table of them does), R must print a number within half a unit of the last digit the page shows, or within one part in a million for a number of over a hundred thousand. The result of the last run: **2,588 of 2,588 numbers on 57 pages**, R stopped on no error and gave no warning, and drew the notebook's 73 charts into one PDF. A unit test also compares the generator written in R with one written in BigInt, draw by draw, for five seeds and four sizes. Where an exact test gives a p-value of exactly .05, or a resample is the cases in another order, the program and the script count it as not below (decision 158). A unit test also asks R to parse the code of every kind of page and option (a different test, a different variable type, a post-hoc test, steps and references of a regression, a cut of the runs test, shares, a control group), and runs only where R is installed.

**What this check shows and what it does not.** It shows that the script prints the numbers the page shows, so a person who runs it gets the page's result from R's own functions, and that the engine and R agree on these data. It does not show that two programs that print the same number are both right for other data (the comparisons with R of each procedure, above, do that, on many more data), and a number can match by chance among the many that R prints (the tolerance is the rounding of the page, a few thousandths, so a chance match of a whole table is not plausible, but a single number could be). **Where the page is more exact than R.** `fisher.test` solves the odds ratio's interval with a tolerance of about 1e-4 of its search, so its limit can differ from the exact one in the third or fourth digit (the page's 94.01 against R's 93.93 for one invented table); the script also prints the interval solved to 1e-14, which is the page's. **What the script leaves out.** The bootstrap, because PrismQDA's resamples come from its own generator and R's `sample` cannot repeat them; the charts.

### Linear regression (`crates/prismqda-stats/src/regression.rs`)

`tests/regression_reference.rs` compares six invented models with R's `lm` (reference made by `make_regression_reference.R`, base R 4.5.3): one predictor; three correlated predictors; a case with very high leverage; nine cases and two predictors; error variance that grows with a predictor; and a dummy-coded three-group variable with a covariate. Everything is compared to a relative error of 1e-11.

| What | Reference |
| --- | --- |
| Coefficients, standard errors, t, p; intervals | `summary(lm)$coefficients`, `confint` |
| R squared, adjusted R squared, F, its degrees of freedom and p, residual standard error, the sums of squares | `summary(lm)` |
| Fitted values, residuals, leverage, Cook's distance, internally and externally studentized residuals | `fitted`, `resid`, `hatvalues`, `cooks.distance`, `rstandard`, `rstudent` |
| Standardized coefficients | written out: the coefficient times the predictor's SD over the outcome's SD |
| Variance inflation factors | written out: one over one minus the R squared of the predictor on the others |
| Breusch-Pagan test (Koenker's studentized form) | written out: n times the R squared of the squared residuals on the predictors, which is what `lmtest::bptest` does by default (the package is not installed) |
| Models entered in steps: R squared change, F change, degrees of freedom, p | `anova(m1, m2)` |

The fit is by a QR decomposition (modified Gram-Schmidt with a second pass), not by the normal equations, so correlated predictors do not lose digits. A predictor that is exactly or all but exactly a combination of the others, a constant predictor, an outcome that is the same in every case, and fewer cases than parameters each give a named refusal, not a number. A categorical predictor becomes a 0/1 column for each category but the reference one.

### Logistic regression (`crates/prismqda-stats/src/logistic.rs`)

`tests/logistic_reference.rs` compares five invented models with R's `glm` with the binomial family (reference made by `make_logistic_reference.R`, base R 4.5.3, `glm.control(epsilon = 1e-14, maxit = 100)`): one number predicting (60 cases); a number and a category with three values (120 cases, a block of two columns); four predictors entered in two steps (200 cases); a small sample (28 cases), where the Wald and profile intervals differ most; and a category only (90 cases), where many predicted probabilities are tied. Everything is compared by relative error 1e-8, except the profile-likelihood limits (1e-7, since both sides find them by root finding).

| What | Reference |
| --- | --- |
| Coefficients, standard errors, z, p | `summary(glm)$coefficients` |
| Wald intervals; odds ratios | `confint.default`; e to the power of the coefficient |
| Profile-likelihood limits | written out: the coefficient value at which the deviance, with that coefficient held fixed (`glm.fit` with an offset), has risen by `qchisq(level, 1)`, found by `uniroot` to 1e-13. `confint(glm)` is R's own profile interval, which it reads off a spline through a grid, so it agrees with the exact one only to about three digits; it is kept as a loose check (within 1 percent) |
| Deviance, null deviance, log likelihood, AIC, BIC | `deviance`, `glm$null.deviance`, `logLik`, `AIC`, `BIC` |
| Likelihood-ratio test of the model; leaving one predictor or one variable's columns out; the change between models in steps | the difference of two `glm` deviances against `pchisq`; `anova(m1, m2, test = "Chisq")` |
| McFadden, Cox and Snell, Nagelkerke | written out from the log likelihoods |
| Hosmer-Lemeshow test and its groups | written out as `ResourceSelection::hoslem.test` does it (the package is not installed): break points at the quantiles of the predicted probabilities (type 7, duplicates dropped), groups by `cut` with the lowest including its low end, groups minus two degrees of freedom |
| Classification table at .5; area under the curve | written out; from the ranks of the predicted probabilities |
| Predicted probabilities, leverage, Cook's distance, Pearson and deviance residuals, standardized residuals | `fitted`, `hatvalues`, `cooks.distance`, `residuals(type = "pearson")`, `residuals(type = "deviance")`, `rstandard` |

**Two things the comparison taught.** R takes the weights behind the standard errors from the iterate before its last, so at its own stopping rule they are off in the eighth digit; the reference is therefore refitted starting from the maximum, and the engine takes one more step at the maximum for the same reason (without that, the standard errors differed by 3 parts in 10 million). And when the coefficient of one predictor is held far from its estimate (which the profile interval does), a plain iteration can run away; a step that would raise the deviance is halved, which changes nothing for a model that is fitted from the start. **What this does not show.** The comparison is with R, not with SPSS, whose Wald statistic is the square of z and whose iteration differs; the Hosmer-Lemeshow groups depend on the way the break points are cut, which differs between programs; a model on data that are nearly separated is refused by a rule (a fitted probability within 10 machine epsilons of 0 or 1, or no settling in 100 steps) that R itself only warns about.

### Curve fits (`crates/prismqda-stats/src/curve.rs`)

`tests/curve_reference.rs` compares all eight curves on three invented sets of data (30, 25, and 40 cases: growth that speeds up, a rise that levels off, a fall toward a floor) with R's `lm` of the transformed variables (reference made by `make_curve_reference.R`, base R 4.5.3): `y ~ x`, `y ~ log(x)`, `y ~ I(1 / x)`, `y ~ x + I(x^2)`, `y ~ x + I(x^2) + I(x^3)`, `log(y) ~ log(x)`, `log(y) ~ x`, and `log(y) ~ I(1 / x)`. Compared by relative error 1e-11: the coefficients, standard errors, t, p, and intervals; R squared, adjusted R squared, F with its degrees of freedom and p, and the residual standard error; and, written out in plain R from what each curve predicts (the exponential of the fitted values for the three curves fitted on the logarithm), the **R squared and the root mean squared error on the scale of the outcome**, and the predictions themselves. The refusals (a logarithm of a number that is not above zero, a reciprocal of zero, a logarithm of an outcome that is not above zero, an outcome that does not vary, a predictor with too few different values, too few cases, a value that is not a number) are tested for their plain errors. What this does not show: the compound and growth curves of the statistical packages are not offered (decision 145), and the logistic curve with an upper bound is not built.

### The bootstrap (`crates/prismqda-stats/src/bootstrap.rs`)

R's random numbers are not PrismQDA's, so the resamples themselves cannot be compared with R's. What can be compared is everything after them, and `tests/bootstrap_reference.rs` does: the reference file holds the resampled values R's `boot` drew (999, 1999, and 199 of them, for the mean and the median of a skewed variable, a standard deviation, a correlation of pairs, a mean difference of pairs, and a small sample) with R's own answers from `boot.ci`. Given those values PrismQDA must give:

| What | Reference | Compared to |
| --- | --- | --- |
| Bootstrap standard error | `sd` of the resampled values | 1e-11 |
| Bias | `mean` of the resampled values minus the estimate | 1e-9 |
| Percentile interval (read off the sorted values the way `boot:::norm.inter` does, interpolating on the normal scale) | `boot.ci(type = "perc")` | 1e-9 |
| BCa interval | `boot.ci(type = "bca", L = ...)` | 1e-9 |
| Jackknife influence values behind BCa | written out: `(n - 1) * (estimate - estimate without case i)` | 1e-9 |

What the resampling itself promises is tested as properties: the same seed gives the same intervals; the bootstrap standard error of a mean is the plug-in formula's to within 3 percent over 10,000 resamples; and a 90 percent interval from 1,000 resamples covers the true mean 84 to 96 percent of the time over 300 invented studies (percentile and BCa both). The seed is SplitMix64, whose sequence is pinned by a test, so a saved seed gives the same intervals on every computer and in every later version. BCa is offered when one set of cases is resampled (a mean, a median, a correlation of pairs, the coefficients of a regression) and not when groups are resampled separately (a difference of two group means gives the percentile interval only). A resample in which the statistic is undefined is counted as a failure and reported, never hidden.

The bootstrap of a whole analysis (decision 146) is not compared with R, since R's resamples are not PrismQDA's, and its result for each number is the same percentile interval whose arithmetic is compared above. What is tested is what it promises (`crates/prismqda-engine/tests/statistics.rs`): that the estimate of each number is the number on the cases as they are; that the same seed gives the same intervals and another seed does not; that a number that is a p-value says how often it fell below .05 and one that is not does not; that it runs for the procedures that have no statistic of their own (the analysis of variance, Kruskal-Wallis, regression); that resampling whole groups gives other intervals than resampling cases, counts the groups, and is refused in words for a variable with fewer than five; and that a cluster alone implies the whole analysis. The browser check runs it for the analysis of variance, a t test, and a t test by groups of cases. What is not tested is the coverage of its intervals over many invented studies, which the bootstrap of a statistic is.

### The notebook kept in the project (`crates/prismqda-engine/src/stat_notebook.rs`)

Nothing here is a number compared with R, so what is tested is what the notebook promises (`crates/prismqda-engine/tests/statistics.rs`, `tests/migration.rs`, and the browser check): that a page keeps exactly the request and the result it was given and comes back in the same order, with its note and section, when the project is closed and opened; that a page of notes has a title and a body and an analysis does not; that pages move one place, and a page can be removed; that running every analysis again after a value changed reports which changed, keeps the earlier result exactly, and leaves a page of notes alone; that a page whose variable was deleted keeps its result and says why while the others still run; that no more than twenty earlier runs are kept; and that a project at an earlier format gains the two tables when it is opened (every earlier version's migration test, with the new tables dropped to make the old file). The browser check types a note, a section, and a page of notes, moves a page, opens the page again, changes a score in the project, runs everything again, and reads the two results side by side.

### The numbers behind the charts (`crates/prismqda-stats/src/charts.rs`)

A chart drawn from wrong numbers misleads as surely as a wrong test, so the numbers behind each chart are compared with R (`tests/charts_reference.rs`, reference made by `make_charts_reference.R`, base R 4.5.3) on seven invented samples: a small one, a skewed one, one of seven values, one with outliers on both sides, one with decimals that sit on bin edges, one with large numbers, and one in which every value is the same.

| What | Reference | Compared to |
| --- | --- | --- |
| Counts in each bin of a histogram | `hist(x, breaks = edges, right = TRUE, include.lowest = TRUE)$counts` | exact |
| The bin edges | PrismQDA's rule (Sturges' number of bins; the width rounded up to 1, 2, 5, or 10 times a power of ten; edges rounded to 12 digits), written out a second time in R | exact |
| The box: quartiles and median, whiskers, outliers, mean | `quantile(type = 7)` with the 1.5 interquartile range rule of `boxplot.stats` applied to those quartiles | 1e-12 |
| Normal probability plot: the points | `qqnorm` (`qnorm(ppoints(n))` against the sorted values) | 1e-11 |
| Normal probability plot: the line | the rule of `qqline` (through the quartiles) | 1e-11 |

The bin edges are PrismQDA's own rule and R's counting is the independent check; a bar is the cases above its lower edge up to and including its upper edge, and the first bar includes its lower edge, as in `hist`. The least-squares line of a scatter is the regression of the section above and is also tested against a worked example.

### The formula language and the transformations (`crates/prismqda-stats/src/formula.rs`, `transform.rs`)

The data editor computes a new variable from a formula. The language is a subset of R's with R's precedence (so `-a ^ 2` is minus a squared, `!a > 2` is not (a greater than 2), and `2 ^ 3 ^ 2` is 512), R's missing values (a missing operand makes the result missing, except that `FALSE & NA` is `FALSE` and `TRUE | NA` is `TRUE`, and `NA ^ 0` is 1), R's `%%`, `%/%`, and `%in%`, backticks for names with spaces, and R's `round`, which is not the round half up that most languages have. Because the text is R, R can evaluate it: `tests/formula_reference.rs` gives 77 formulas to R (`eval(parse(text = formula), data)`, reference made by `make_formula_reference.R`, base R 4.5.3) over 24 invented cases with missing values, text, a name with a space, zeros, negatives, and halves, and requires the same answer for every case (1,848 values) to 1e-12: the same numbers, the same truth values, the same texts, and a missing value wherever R has one.

| What | Reference | Compared to |
| --- | --- | --- |
| Arithmetic, comparisons, logic, `%in%`, the one-argument functions, `log` with a base, `pmin` and `pmax`, `ifelse`, `is.na`, `as.numeric` | R evaluates the same text | 1e-12 |
| `round(x, digits)` on halves and on numbers whose binary form falls either side of a half (0.5, 2.5, 2.675, 1.005, 0.15, 2.345, 10.555, and others), 0 to 3 digits | `round` | 1e-12 |
| `rowmean`, `rowsum`, `rowmin`, `rowmax`, `nmiss` (across several variables of one case) | written out with `rowMeans`, `rowSums`, `apply`, and `is.na`, with a case whose values are all missing given a missing result, not R's 0 for `rowSums` | 1e-12 |
| Standardizing (z-scores) | `scale(x)` | 1e-12 |
| Ranking (ties share the average), lowest first or highest first | `rank(x, na.last = "keep")`, `rank(-x)` | exact |
| Cutting into ranges, upper edge included or lower edge included | `as.integer(cut(x, breaks, right, include.lowest = TRUE))` | exact |

Text comparison with `<` and `>` is by character code, not by the language's sorting order (R sorts by the locale); only `==`, `!=`, and `%in%` on texts are used on data that way, and they are R's. A value that a formula makes infinite or not a number is stored as missing.

### The data editor's later tools (`crates/prismqda-engine/src/dataprep.rs`, `datatable.rs`, `statistics.rs`)

| What | Reference | Compared to |
| --- | --- | --- |
| The year, quarter, month, ISO week, day, day of the year, and weekday of 24 dates (including the days around New Year in several years, leap days, and 1970) | `format(as.Date(x), "%Y")`, `%m`, `%V`, `%d`, `%j`, `%u` (made by `tests/reference/make_dataprep_reference.R`, base R 4.5.3; tested in `tests/dataprep_reference.rs`) | exact |
| The mean, sum, median, smallest, largest, standard deviation, and count of 12 values in 3 groups, given to each case of its group | `ave(x, g, FUN = mean)` and the same with `sum`, `median`, `min`, `max`, `sd`, `length` | 1e-12 |
| A weighted analysis (describe, Welch's t test, Pearson correlation, Mann-Whitney) | the same analysis of a project that holds the cases written out that many times (`tests/data_tools.rs`), and, for the recorded script, R's own `d[rep(seq_len(nrow(d)), w), ]` followed by the same R code (the interface check runs the script in R and compares every number on every page, including a weighted one) | exact (the engine); the script's usual tolerance |

Not compared with R, and checked another way: the random sample (it has the size asked for, never an ineligible case, shares a count among groups so that the shares add up to it, and is the same for the same seed; R's `sample` uses another generator, so a sample cannot be repeated in R); sorting (the order is checked by hand on cases with ties, missing values, dates, and names); and the tables stood on end and laid across (a table laid across again is the table the cases began as; two rows for one who and observation are refused and named). Weights are frequency weights only: SPSS and R use other meanings for a weight that is not a count (an inverse-probability weight, a survey weight), and none of those is built.

### Joint displays (`crates/prismqda-engine/src/explore/joint.rs`)

The typology table's n, mean, and standard deviation come from `describe::mean` and `describe::variance`, the functions of the descriptives that are compared with R above. The engine tests check the table's numbers against sums worked out by hand (group means of 14.75 and 35.33, a standard deviation of the square root of 82.75 over 3, counts of categories), that a value that is missing or not a number is left out of n, and that a group of one has a mean and no spread. The case-ordered display's excerpt counts are tested against the code matrix's for every case, and its order (numbers by number, words by words, missing last both ways) against cases worked out by hand. The browser check repeats a typology table on eight invented respondents with every figure worked out by hand. **Not compared with R as a whole**: the grouping and the ordering are the program's own rules, which the tests and the decision (150) state.

The **deviation table's residual** is the adjusted standardized residual of the two by two table of in the group or not and with the code or not. The engine test compares six of them with R 4.5.3's `chisq.test(matrix(c(a, row - a, col - a, n - row - col + a), 2), correct = FALSE)$stdres[1, 1]`, to 1e-9 (for example 0.710705312219, -0.592254426849, and -1.535298947157), and the browser check compares the North and South groups of its eight invented respondents with R's 2.190890230021. **Similarity** is Gower's measure worked out by hand and again in R's arithmetic for the pairs the browser check names (0.982456140351, 0.929824561404, 0.868421052632); it is not compared with a package, since the standard one is not in base R. The side-by-side and bubble displays are counts and means (the means are checked against R's `mean()` and `sd()` in the browser check) and are tested against the code matrix's counts for the same code.

### Samples and intervals (not against R)

- **The sample drawer** (random and stratified draws, by seed) is tested for what it promises: the same seed gives the same sample, a stratified draw takes from each group in proportion by largest remainder, and a unit left out of coding is never drawn. These are properties, not values from R.
- **Bootstrap intervals for the agreement coefficients** are percentile intervals from a seeded generator, so the same seed gives the same interval. A test checks that the interval from several coefficients at once equals the interval from each alone. They are not compared with R's bootstrap, which uses a different random generator and cannot give the same resamples. (The general bootstrap in the section above, which is used for the statistics procedures, is compared with R's `boot` once the resampled values are given.)

## What is checked another way

These are tested, but not against an outside reference:

- **Word frequencies and keywords in context** (`crates/prismqda-engine/tests/text_analytics.rs`): counts worked out by hand on small invented documents.
- **Everything the engine counts** for Explore (code matrices, relations, crosstabs, summaries): exact counts on invented projects, and the numbers on a screen are tested to agree with the evidence behind them.
- **Agreement on pictures**: cell counts worked out by hand on a 16 by 16 grid. The method itself (cells that are not independent) is explained in the written report it produces; it is a guide and has no reference to be validated against.
- **Project exchange (QDPX)** (`crates/prismqda-export/tests/qdpx.rs`, `crates/prismqda-api/tests/api_qdpx.rs`, `tools/qdpx`): an export of a test project is valid against the standard's own project schema (rebuilt from the text of the REFI-QDA 1.5 specification; see `tools/qdpx/README.md`); a project written and read back keeps its codes and their tree, coders, texts letter for letter (with a letter outside the basic plane in them, so that a count in bytes or UTF-16 units would fail), every coding on the same words from the same coder, a picture's coded area, cases and variables, memos, and sets; a file made by hand in the shape of the standard's example is read with its selection of 56 characters from 139 to 195 on exactly those characters; a text not in Unicode normal form keeps its codings on the same words; every identifier in a file is its own. **Not tried:** another program opening our file, or our reading its files.

## What is not validated, because it is not built

Factorial analysis of variance, scales, factor and cluster analysis, and the joint displays of Mixed Methods are not built yet, and neither are the topic and sentiment parts of Text Analytics. Each procedure will be added here with its R reference, tolerance, and datasets before it is offered.

## How to run it again

```bash
cargo test -p prismqda-stats
```

To remake the reference values (the agreement script needs R with `irr`, `irrCAC`, and `jsonlite`; the other two need only base R and `jsonlite`):

```bash
Rscript crates/prismqda-stats/tests/reference/make_agreement_reference.R
Rscript crates/prismqda-stats/tests/reference/make_distribution_reference.R
Rscript crates/prismqda-stats/tests/reference/make_procedures_reference.R
Rscript crates/prismqda-stats/tests/reference/make_posthoc_reference.R
Rscript crates/prismqda-stats/tests/reference/make_ordinal_reference.R
Rscript crates/prismqda-stats/tests/reference/make_nonparametric_reference.R
```

The whole Rust suite (`cargo test --workspace`) runs this check with every build.

## What this does not tell you

A coefficient that is right is not a coding that is good. The numbers say how the coders' choices relate to each other; they do not say whether the codes mean what the study needs them to mean, and a high figure on a code that almost nothing carries (the kappa paradox) can mislead. The report PrismQDA writes says so where it matters.
