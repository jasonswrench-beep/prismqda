// R code for the bootstrap of a page: PrismQDA draws its resamples with SplitMix64 (a fixed algorithm, decision 25), and the script
// writes the same generator out in R, so the resamples, and with them the intervals, are the ones the page shows. Nothing in this file
// is words for the person, so it is not audited for language (see scripts/i18n/ast.mjs).
//
// Two kinds of bootstrap are written: the bootstrap of the statistic of a page (the mean, a difference, a correlation, the coefficients
// of a regression), and the bootstrap of the whole analysis (the numbers a page leads with, worked out again on every resample of the
// cases or of whole groups of cases). Both draw the same way as the engine: for each resample the same number of draws, in the same
// order, from the one seeded stream.

import { asNum, asVal, col, grouped, level, q, rn, take, vec, modelSetup, type Helper, type RPage } from './rbase.code';

export const BOOT_HELPERS: Record<string, Helper> = {
  pq_draws: {
    code: `# SplitMix64 (the generator PrismQDA draws its resamples with), written in pieces of 16 bits so that R's numbers hold every product exactly.
# Gives the draws first + 1 to first + count of the stream of a seed, each a whole number from 1 to m.
pq_draws <- function(seed, first, count, m) {
  K <- 65536
  i <- first + seq_len(count)
  g <- c(0x7C15, 0x7F4A, 0x79B9, 0x9E37)
  st <- list(); carry <- 0
  for (k in 1:4) { v <- (seed %/% K^(k - 1)) %% K + i * g[k] + carry; st[[k]] <- v %% K; carry <- v %/% K }
  xs <- function(z, sh) {
    q <- sh %/% 16; r <- sh %% 16; out <- z
    for (k in 1:4) {
      a <- if (k + q <= 4) z[[k + q]] else 0
      b <- if (k + q + 1 <= 4) z[[k + q + 1]] else 0
      out[[k]] <- bitwXor(as.integer(z[[k]]), as.integer(a %/% 2^r + (b %% 2^r) * 2^(16 - r)))
    }
    out
  }
  mul <- function(z, cl) {
    out <- list(); carry <- 0
    for (k in 1:4) {
      v <- carry
      for (j in 1:k) v <- v + z[[j]] * cl[k - j + 1]
      out[[k]] <- v %% K; carry <- v %/% K
    }
    out
  }
  z <- mul(xs(st, 30), c(0xE5B9, 0x1CE4, 0x476D, 0xBF58))
  z <- mul(xs(z, 27), c(0x11EB, 0x1331, 0x49BB, 0x94D0))
  z <- xs(z, 31)
  hi <- z[[4]] * K + z[[3]]; lo <- z[[2]] * K + z[[1]]
  floor((hi * m + floor(lo * m / 4294967296)) / 4294967296) + 1
}`,
  },
  pq_interval: {
    code: `# The intervals of one statistic from its resampled values, the way PrismQDA (and R's boot package) read them off.
pq_interval <- function(est, reps, level, influence = NULL) {
  reps <- sort(reps[is.finite(reps)]); r <- length(reps)
  out <- c(estimate = est, bias = NA_real_, se = NA_real_, lower = NA_real_, upper = NA_real_, bca_lower = NA_real_, bca_upper = NA_real_, resamples = r)
  if (r < 20) return(out)
  inter <- function(alpha) {
    rk <- (r + 1) * alpha; k <- if (is.nan(rk)) 0 else trunc(rk)
    if (k < 1) return(reps[1])
    if (k >= r) return(reps[r])
    if (k == rk) return(reps[k])
    z <- qnorm(c(alpha, k / (r + 1), (k + 1) / (r + 1)))
    reps[k] + (z[1] - z[2]) / (z[3] - z[2]) * (reps[k + 1] - reps[k])
  }
  a <- c((1 - level) / 2, (1 + level) / 2)
  out[c("bias", "se", "lower", "upper")] <- c(mean(reps) - est, sd(reps), inter(a[1]), inter(a[2]))
  if (!is.null(influence)) {
    # A resample that equals the estimate up to rounding (the cases in another order) is not below it, whatever the rounding says.
    w <- qnorm(mean(reps < est - 1e-9 * max(1, abs(est)))); acc <- sum(influence^3) / (6 * sum(influence^2)^1.5)
    if (is.finite(w) && is.finite(acc)) {
      adj <- function(al) { z <- qnorm(al); pnorm(w + (w + z) / (1 - acc * (w + z))) }
      out[c("bca_lower", "bca_upper")] <- c(inter(adj(a[1])), inter(adj(a[2])))
    }
  }
  out
}`,
    needs: [],
  },
  'print.pq_table': {
    code: `# A table of bootstrap intervals is printed with enough digits to be compared with the page, whatever the size of its numbers.
print.pq_table <- function(x, ...) { print(unclass(x), digits = 12); invisible(x) }`,
  },
  pq_logit: {
    code: `# The maximum likelihood fit of a logistic regression by iteratively reweighted least squares, step for step as PrismQDA does it (the same start, the same
# stop, one more step once the deviance has settled), so that a resample is left out, because the outcome is separated or the fit does not settle, exactly when
# PrismQDA leaves it out. Gives the coefficients with their standard errors, the fitted probabilities, and the deviance, or NULL.
pq_logit <- function(X, y) {
  n <- length(y); p <- ncol(X)
  if (n <= p || sum(y == 1) == 0 || sum(y == 0) == 0) return(NULL)
  m0 <- (y + 0.5) / 2; eta <- log(m0 / (1 - m0))
  dev_of <- function(e) 2 * sum(pmax(ifelse(y == 1, -e, e), 0) + log1p(exp(-abs(e))))
  dev_old <- dev_of(eta); prev <- NULL; settled <- FALSE
  for (it in 1:100) {
    mu <- 1 / (1 + exp(-eta)); w <- pmax(mu * (1 - mu), .Machine$double.xmin); sw <- sqrt(w)
    f <- lm.fit(X * sw, (eta + (y - mu) / w) * sw)
    if (anyNA(f$coefficients)) return(NULL)
    coef <- f$coefficients; eta <- drop(X %*% coef); dev <- dev_of(eta)
    if (!is.null(prev)) {
      step <- 1; halvings <- 0
      while ((!is.finite(dev) || dev > dev_old + 1e-12 * (abs(dev_old) + 0.1)) && halvings < 40) {
        step <- step / 2; halvings <- halvings + 1
        coef <- prev + step * (f$coefficients - prev); eta <- drop(X %*% coef); dev <- dev_of(eta)
      }
    }
    if (!is.finite(dev) || any(!is.finite(eta))) return(NULL)
    if (settled) {
      mu <- 1 / (1 + exp(-eta)); edge <- 10 * .Machine$double.eps
      if (any(mu < edge | mu > 1 - edge)) return(NULL)
      return(list(coef = coef, se = sqrt(diag(chol2inv(f$qr$qr[seq_len(p), seq_len(p), drop = FALSE]))), mu = mu, deviance = dev))
    }
    settled <- abs(dev - dev_old) / (abs(dev) + 0.1) < 1e-12
    dev_old <- dev; prev <- coef
  }
  NULL
}`,
  },
  pq_boot: {
    code: `# Bootstraps the statistics of one set of n cases. stat(i) is given the numbers of the cases to use and gives the statistics (a named vector), or NULL.
pq_boot <- function(n, stat, seed, resamples, level) {
  idx <- seq_len(n); est <- stat(idx)
  if (n < 3 || length(est) == 0) return(NULL)
  m <- length(est); reps <- matrix(NA_real_, resamples, m)
  for (r in seq_len(resamples)) {
    v <- stat(pq_draws(seed, (r - 1) * n, n, n))
    if (length(v) == m) reps[r, ] <- v
  }
  infl <- matrix(NA_real_, n, m)
  for (i in idx) {
    v <- stat(idx[-i])
    if (length(v) != m) { infl <- NULL; break }
    infl[i, ] <- (n - 1) * (est - v)
  }
  out <- t(vapply(seq_len(m), function(k) pq_interval(est[k], reps[, k], level, if (is.null(infl)) NULL else infl[, k]), numeric(8)))
  rownames(out) <- names(est); structure(out, class = 'pq_table')
}`,
    needs: ['pq_draws', 'pq_interval', 'print.pq_table'],
  },
  pq_boot_groups: {
    code: `# The same for a statistic of several groups, drawn within each group so that every resample has the groups' own sizes (no BCa interval).
pq_boot_groups <- function(sizes, stat, seed, resamples, level) {
  est <- stat(lapply(sizes, seq_len))
  if (any(sizes < 2) || length(est) != 1) return(NULL)
  start <- cumsum(c(0, sizes)); reps <- rep(NA_real_, resamples)
  for (r in seq_len(resamples)) {
    v <- stat(lapply(seq_along(sizes), function(j) pq_draws(seed, (r - 1) * sum(sizes) + start[j], sizes[j], sizes[j])))
    if (length(v) == 1) reps[r] <- v
  }
  out <- t(as.matrix(pq_interval(est, reps, level))); rownames(out) <- names(est); structure(out, class = 'pq_table')
}`,
    needs: ['pq_draws', 'pq_interval', 'print.pq_table'],
  },
  pq_boot_whole: {
    code: `# Bootstraps a whole analysis: headline(d) gives the numbers the page leads with, and is run again on each resample of the cases (or, with
# cluster, of the groups of cases that share a value). A p-value is also told by the share of resamples in which it fell below .05.
pq_boot_whole <- function(d, headline, labels, is_p, seed, resamples, level, cluster = NULL) {
  n <- nrow(d); est <- headline(d); groups <- NULL
  if (!is.null(cluster)) {
    key <- ifelse(is.na(cluster), paste0("\\001case ", seq_len(n) - 1), cluster)
    groups <- split(seq_len(n), factor(key, levels = unique(sort(key, method = "radix"))))
  }
  size <- if (is.null(groups)) n else length(groups)
  reps <- matrix(NA_real_, resamples, length(est)); failed <- 0
  for (r in seq_len(resamples)) {
    pick <- pq_draws(seed, (r - 1) * size, size, size)
    rows <- if (is.null(groups)) pick else unlist(groups[pick], use.names = FALSE)
    v <- tryCatch(suppressWarnings(headline(d[rows, , drop = FALSE])), error = function(e) NULL)
    if (is.null(v) || length(v) != length(est)) failed <- failed + 1 else reps[r, ] <- v
  }
  keep <- which(is.finite(est))
  if (length(keep) == 0) { cat("none of the numbers could be worked out\\n"); return(invisible(NULL)) }
  out <- t(vapply(keep, function(k) pq_interval(est[k], reps[, k], level), numeric(8)))
  below <- vapply(keep, function(k) if (is_p[k]) mean(reps[is.finite(reps[, k]), k] < 0.05 - 1e-9) else NA_real_, numeric(1))
  res <- data.frame(statistic = labels[keep], estimate = out[, "estimate"], se = out[, "se"], lower = out[, "lower"], upper = out[, "upper"], percent_p_below_05 = 100 * below)
  res <- res[!is.na(res$se), , drop = FALSE]; rownames(res) <- NULL
  print(res, digits = 12)
  cat("resamples that could not be analyzed:", failed, "\\n")
  invisible(res)
}`,
    needs: ['pq_draws', 'pq_interval'],
  },
};

/** The seed, the number of resamples, and the level, as the page used them. */
function plan(p: RPage): string {
  const b = p.options.bootstrap;
  return `${rn(b?.seed ?? 1)}, ${rn(b?.resamples ?? 2000)}, ${level(p.options)}`;
}

/** The bootstrap of the statistic of a page: the same resamples as the page, so the same intervals. Empty when the page has none. */
export function bootCode(p: RPage): string[] {
  const b = p.options.bootstrap;
  if (!b || b.analysis || b.cluster) return [];
  const args = plan(p);
  const one = (stat: string) => [`print(pq_boot(length(x), function(i) { ${stat} }, ${args}))`];
  const dd = [`dd <- x - y`];
  const corr = (method: string) => [`print(pq_boot(length(x), function(i) c(correlation = suppressWarnings(cor(x[i], y[i], method = ${q(method)}))), ${args}))`];
  switch (p.procedure) {
    case 'describe':
      return p.kind === 'frequencies' ? [] : one('v <- x[i]; c(mean = mean(v), median = median(v), sd = sd(v))');
    case 'one_sample_t':
      return one('c(mean = mean(x[i]))');
    case 'paired_t':
      return [...dd, `print(pq_boot(length(dd), function(i) c(mean = mean(dd[i])), ${args}))`];
    case 'wilcoxon':
      return p.vars.length === 2 ? [...dd, `print(pq_boot(length(dd), function(i) c(median = median(dd[i])), ${args}))`] : one('c(median = median(x[i]))');
    case 'independent_t':
      return [`print(pq_boot_groups(c(length(a), length(b)), function(i) c(mean_difference = mean(a[i[[1]]]) - mean(b[i[[2]]])), ${args}))`];
    case 'mann_whitney':
      return [`print(pq_boot_groups(c(length(a), length(b)), function(i) c(median_difference = median(a[i[[1]]]) - median(b[i[[2]]])), ${args}))`];
    case 'pearson':
    case 'spearman':
    case 'kendall':
      return corr(p.procedure);
    case 'linear_regression': {
      const fit = modelSetup(p, false).last;
      return [
        `Xb <- model.matrix(${fit}); yb <- ${fit}$model[[1]]`,
        `print(pq_boot(length(yb), function(i) { tss <- sum((yb[i] - mean(yb[i]))^2); f <- lm.fit(Xb[i, , drop = FALSE], yb[i], tol = 1e-10); if (length(i) <= ncol(Xb) || tss == 0 || anyNA(f$coefficients)) return(NULL); c(f$coefficients, r_squared = 1 - sum(f$residuals^2) / tss) }, ${args}))`,
      ];
    }
    case 'logistic_regression': {
      const fit = modelSetup(p, true).last;
      return [
        `Xb <- model.matrix(${fit}); yb <- ${fit}$y`,
        `print(pq_boot(length(yb), function(i) { f <- pq_logit(Xb[i, , drop = FALSE], yb[i]); if (is.null(f)) return(NULL); n1 <- sum(yb[i] == 1); r <- rank(f$mu); c(setNames(f$coef, colnames(Xb)), auc = (sum(r[yb[i] == 1]) - n1 * (n1 + 1) / 2) / (n1 * (length(i) - n1))) }, ${args}))`,
      ];
    }
    default:
      return [];
  }
}

// ---------------------------------------------------------------- the bootstrap of a whole analysis

/** The R that works out, from the data frame d, the numbers a page leads with: the lines that prepare the cases, and the expression of each number by its path. */
interface Headline {
  prep: string[];
  expr: (path: string, key?: string) => string | undefined;
}

const pTest = (name: string) => `${name}$p.value`;

/** A resample in which only one group is left cannot be compared, and PrismQDA does not analyze it. */
const dropEmpty = ['g <- droplevels(g)', 'if (nlevels(g) < 2) stop("fewer than two groups")'];

function headline(p: RPage): Headline | undefined {
  const o = p.options;
  const [v0, v1] = p.vars;
  const lvl = level(o);
  const mu = rn(o.mu ?? 0);
  const fixed = (table: Record<string, string>): Headline['expr'] => (path) => table[path];
  const groupsPrep = () => [...grouped(p, false), ...dropEmpty, 'a <- y[g == levels(g)[1]]; b <- y[g == levels(g)[2]]'];
  switch (p.procedure) {
    case 'describe':
      if (p.kind === 'frequencies') return undefined;
      return {
        prep: take([['x', asNum(v0)]]),
        expr: fixed({ 'descriptives/mean': 'mean(x)', 'descriptives/median': 'median(x)', 'descriptives/sd': 'sd(x)', 'descriptives/skewness': 'skew(x)', 'descriptives/kurtosis': 'kurt(x)' }),
      };
    case 'one_sample_t':
      return {
        prep: [...take([['x', asNum(v0)]]), `tt <- t.test(x, mu = ${mu})`, `dz <- (mean(x) - ${mu}) / sd(x)`],
        expr: fixed({ 't_test/estimate': 'unname(tt$estimate)', 't_test/t': 'unname(tt$statistic)', 't_test/p': pTest('tt'), 't_test/d': 'dz', 't_test/g': 'dz * hedges_j(length(x) - 1)' }),
      };
    case 'paired_t':
      return {
        prep: [...take([['x', asNum(v0)], ['y', asNum(v1)]]), 'tt <- t.test(x, y, paired = TRUE)', 'dz <- mean(x - y) / sd(x - y)'],
        expr: fixed({ 't_test/estimate': 'unname(tt$estimate)', 't_test/t': 'unname(tt$statistic)', 't_test/p': pTest('tt'), 't_test/d': 'dz', 't_test/g': 'dz * hedges_j(length(x) - 1)' }),
      };
    case 'independent_t':
      return {
        prep: [
          ...groupsPrep(),
          `tt <- t.test(a, b, var.equal = ${o.equal_variances ? 'TRUE' : 'FALSE'})`,
          'n1 <- length(a); n2 <- length(b); sp <- sqrt(((n1 - 1) * var(a) + (n2 - 1) * var(b)) / (n1 + n2 - 2)); dd <- (mean(a) - mean(b)) / sp',
        ],
        expr: fixed({ 't_test/estimate': 'mean(a) - mean(b)', 't_test/t': 'unname(tt$statistic)', 't_test/p': pTest('tt'), 't_test/d': 'dd', 't_test/g': 'dd * hedges_j(n1 + n2 - 2)' }),
      };
    case 'one_way_anova':
      return {
        prep: [
          ...grouped(p, false),
          ...dropEmpty,
          // PrismQDA gives up when there are no more cases than groups, or when the cases of every group are alike (R's lm would leave a sum of squares of about 1e-30 and an F of about 1e31).
          'if (length(y) <= nlevels(g) || sum(tapply(y, g, function(v) sum((v - mean(v))^2))) == 0) stop("too few cases, or the cases of every group are alike")',
          'a <- anova(lm(y ~ g)); ss <- a[["Sum Sq"]]; msw <- ss[2] / a$Df[2]',
        ],
        expr: fixed({
          'anova/test/f': 'a[["F value"]][1]',
          'anova/test/p': 'a[["Pr(>F)"]][1]',
          'anova/eta_squared': 'ss[1] / sum(ss)',
          'anova/omega_squared': '(ss[1] - a$Df[1] * msw) / (sum(ss) + msw)',
        }),
      };
    case 'kruskal_wallis':
      return {
        prep: [...grouped(p, false), ...dropEmpty, 'k <- kruskal.test(y ~ g)'],
        expr: fixed({ 'kruskal_wallis/h': 'unname(k$statistic)', 'kruskal_wallis/p': pTest('k'), 'kruskal_wallis/epsilon_squared': 'unname(k$statistic) / (length(y) - 1)' }),
      };
    case 'mann_whitney':
      return {
        prep: [
          ...groupsPrep(),
          'w <- suppressWarnings(wilcox.test(a, b)); u <- unname(w$statistic); nn <- length(a) * length(b); N <- length(a) + length(b)',
          'ties <- table(c(a, b)); exact <- length(a) < 50 && length(b) < 50 && all(ties == 1)',
          'sigma <- sqrt(nn / 12 * ((N + 1) - sum(ties^3 - ties) / (N * (N - 1)))); cen <- u - nn / 2',
        ],
        expr: fixed({
          'mann_whitney/u': 'u',
          'mann_whitney/z': 'if (exact) NA_real_ else (cen - 0.5 * sign(cen)) / sigma',
          'mann_whitney/p': pTest('w'),
          'mann_whitney/rank_biserial': '2 * u / nn - 1',
          'mann_whitney/probability_of_superiority': 'u / nn',
        }),
      };
    case 'wilcoxon': {
      const prep =
        p.vars.length === 2
          ? [...take([['x', asNum(v0)], ['y', asNum(v1)]]), 'w <- suppressWarnings(wilcox.test(x, y, paired = TRUE)); sr <- signed_rank(x - y); nz <- (x - y)[x - y != 0]']
          : [...take([['x', asNum(v0)]]), `w <- suppressWarnings(wilcox.test(x, mu = ${mu})); sr <- signed_rank(x - ${mu}); nz <- (x - ${mu})[x - ${mu} != 0]`];
      return {
        prep: [...prep, 'exact <- sr[["pairs_used"]] < 50 && !anyDuplicated(abs(nz)) && sr[["equal_pairs"]] == 0'],
        expr: fixed({
          'wilcoxon/v': 'sr[["V"]]',
          'wilcoxon/z': 'if (exact) NA_real_ else sr[["z_normal_approximation"]]',
          'wilcoxon/p': pTest('w'),
          'wilcoxon/rank_biserial': 'sr[["rank_biserial"]]',
        }),
      };
    }
    case 'chi_square':
    case 'fisher': {
      const prep = [
        `xr <- ${asVal(v0)}; xc <- ${asVal(v1)}; ok <- !is.na(xr) & !is.na(xc)`,
        `tab <- table(factor(xr[ok], levels = ${vec(p.rowValues ?? v0.categories)}), factor(xc[ok], levels = ${vec(p.colValues ?? v1.categories)}))`,
        'tab <- tab[rowSums(tab) > 0, colSums(tab) > 0, drop = FALSE]',
      ];
      if (p.procedure === 'fisher') {
        return { prep: [...prep, 'if (!all(dim(tab) == 2)) stop("not a two by two table")'], expr: fixed({ 'fisher/odds_ratio': `fisher_precise(tab, ${lvl})[["odds_ratio"]]`, 'fisher/p': 'fisher.test(tab)$p.value' }) };
      }
      return {
        prep: [...prep, 'if (nrow(tab) < 2 || ncol(tab) < 2) stop("table too small")', 'x <- suppressWarnings(chisq.test(tab, correct = FALSE))'],
        expr: fixed({ 'chi_square/chi2': 'unname(x$statistic)', 'chi_square/p': pTest('x'), 'chi_square/cramers_v': 'chi_extras(tab, x)[["cramers_v"]]' }),
      };
    }
    case 'pearson':
    case 'spearman':
    case 'kendall':
      return {
        prep: [...take([['x', asNum(v0)], ['y', asNum(v1)]]), `ct <- suppressWarnings(cor.test(x, y, method = ${q(p.procedure)}))`],
        expr: fixed({ 'correlation/r': 'unname(ct$estimate)', 'correlation/p': pTest('ct') }),
      };
    case 'shapiro_wilk':
      return { prep: [...take([['x', asNum(v0)]]), 's <- shapiro.test(x)'], expr: fixed({ 'shapiro_wilk/w': 'unname(s$statistic)', 'shapiro_wilk/p': pTest('s') }) };
    case 'ks_normal':
      return {
        prep: [...take([['x', asNum(v0)]]), p.estimated ? 'k <- suppressWarnings(ks.test(x, "pnorm", mean(x), sd(x)))' : `k <- suppressWarnings(ks.test(x, "pnorm", ${rn(p.mean ?? o.mu ?? 0)}, ${rn(p.sd ?? o.sd ?? 1)}))`],
        expr: fixed({ 'ks/d': 'unname(k$statistic)', 'ks/p': pTest('k') }),
      };
    case 'ks_two':
      return { prep: [...groupsPrep(), 'k <- suppressWarnings(ks.test(a, b))'], expr: fixed({ 'ks/d': 'unname(k$statistic)', 'ks/p': pTest('k') }) };
    case 'runs': {
      const prep = v0.numeric
        ? [...take([['x', asNum(v0)]]), `cut <- ${(o.cut ?? 'median') === 'median' ? 'median(x)' : o.cut === 'mean' ? 'mean(x)' : rn(o.mu ?? 0)}`, 'side <- x >= cut']
        : [...take([['x', asVal(v0)]]), `side <- x == ${q(v0.categories[1] ?? '')}`];
      // A resample whose cases all fall on one side of the cut has no change of value to count, and PrismQDA does not analyze it.
      return { prep: [...prep, 'if (length(unique(side)) < 2) stop("every case falls on the same side")', 'r <- runs_test(side)'], expr: fixed({ 'runs/runs': 'r[["runs"]]', 'runs/z': 'r[["z"]]', 'runs/p_exact': 'r[["p_exact"]]', 'runs/p_normal': 'r[["p_normal"]]' }) };
    }
    case 'binomial': {
      // The value that counts as a success is the one the page chose; with none chosen it is the first value there is, on every resample.
      const cats = p.counts ?? v0.categories;
      const pick = o.success !== undefined ? `success <- ${q(o.success)}; if (!any(x == success)) stop("no case has the value that counts as a success")` : `success <- intersect(${vec(cats)}, x)[1]`;
      return {
        prep: [...take([['x', asVal(v0)]]), pick, `bt <- binom.test(sum(x == success), length(x), p = ${rn(o.proportion ?? 0.5)})`],
        expr: fixed({ 'binomial/estimate': 'unname(bt$estimate)', 'binomial/p': pTest('bt') }),
      };
    }
    case 'goodness_of_fit': {
      const cats = p.counts ?? v0.categories;
      const shares = o.expected ? `c(${cats.map((c) => `${q(c)} = ${rn(o.expected?.[c] ?? 0)}`).join(', ')})[lv]` : 'rep(1, length(lv))';
      return {
        prep: [
          ...take([['x', asVal(v0)]]),
          `lv <- intersect(${vec(cats)}, x); if (length(lv) < 2) stop("fewer than two categories")`,
          'tab <- table(factor(x, levels = lv))',
          `shares <- ${shares}; shares <- shares / sum(shares)`,
          'x2 <- suppressWarnings(chisq.test(tab, p = shares))',
        ],
        expr: fixed({ 'goodness_of_fit/chi2': 'unname(x2$statistic)', 'goodness_of_fit/p': pTest('x2') }),
      };
    }
    case 'sign': {
      const prep = p.vars.length === 2 ? [...take([['x', asNum(v0)], ['y', asNum(v1)]]), 'dif <- x - y'] : [...take([['x', asNum(v0)]]), `dif <- x - ${mu}`];
      return {
        prep: [...prep, 'pos <- sum(dif > 0); neg <- sum(dif < 0); bt <- binom.test(pos, pos + neg, 0.5)'],
        expr: fixed({ 'sign/proportion_positive': 'pos / (pos + neg)', 'sign/p': pTest('bt') }),
      };
    }
    case 'friedman':
      return {
        prep: [`m <- cbind(${p.vars.map(asNum).join(', ')})`, 'm <- m[complete.cases(m), , drop = FALSE]', 'f <- friedman.test(m)'],
        expr: fixed({ 'friedman/chi2': 'unname(f$statistic)', 'friedman/p': pTest('f'), 'friedman/kendall_w': 'unname(f$statistic) / (nrow(m) * (ncol(m) - 1))' }),
      };
    case 'median_test':
      return { prep: [...grouped(p, false), ...dropEmpty, 'r <- median_test(y, g)'], expr: fixed({ 'median_test/chi2': 'unname(r$chisq$statistic)', 'median_test/p': 'r$chisq$p.value' }) };
    case 'moses': {
      const control = o.control ?? p.groups[0];
      return {
        prep: [
          ...grouped(p, false),
          `control <- y[g == ${q(control)}]; test <- y[g != ${q(control)}]; if (length(control) < 3 || length(test) < 1) stop("too few cases")`,
          `r <- moses_test(control, test${o.trim !== undefined ? `, ${o.trim}` : ''}); if (is.nan(r[["trimmed_p"]])) stop("too few cases after trimming")`,
        ],
        expr: fixed({ 'moses/p': 'r[["p"]]', 'moses/trimmed_p': 'r[["trimmed_p"]]' }),
      };
    }
    case 'mcnemar': {
      const values = p.values ?? v0.categories;
      return {
        prep: [`xa <- ${asVal(v0)}; xb <- ${asVal(v1)}; ok <- !is.na(xa) & !is.na(xb)`, `tab <- table(factor(xa[ok], levels = ${vec(values)}), factor(xb[ok], levels = ${vec(values)}))`, 'mt <- mcnemar.test(tab)'],
        expr: fixed({ 'mcnemar/chi2_corrected': 'unname(mt$statistic)', 'mcnemar/p_corrected': pTest('mt') }),
      };
    }
    case 'ordinal_association':
      return {
        prep: [
          `xr <- ${asVal(v0)}; xc <- ${asVal(v1)}; ok <- !is.na(xr) & !is.na(xc)`,
          `tab <- table(factor(xr[ok], levels = ${vec(p.rowValues ?? v0.categories)}), factor(xc[ok], levels = ${vec(p.colValues ?? v1.categories)}))`,
          'tab <- tab[rowSums(tab) > 0, colSums(tab) > 0, drop = FALSE]; if (nrow(tab) < 2 || ncol(tab) < 2) stop("table too small")',
          'r <- ordinal_assoc(tab); if (sum(attr(r, "pairs")[c("concordant", "discordant")]) <= 0) stop("no pairs")',
        ],
        expr: fixed({ 'ordinal/gamma/estimate': 'r$estimate[1]', 'ordinal/tau_b/estimate': 'r$estimate[2]', 'ordinal/tau_b/p': 'if (is.finite(r$z[2])) r$p[2] else NA_real_' }),
      };
    case 'two_way_anova': {
      const [, va, vb] = p.vars;
      return {
        prep: [
          ...take([['y', asNum(v0)], ['a', asVal(va)], ['b', asVal(vb)]]),
          `A <- factor(a, levels = ${vec(p.levelsA ?? va.categories)}); B <- factor(b, levels = ${vec(p.levelsB ?? vb.categories)}); A <- droplevels(A); B <- droplevels(B)`,
          // PrismQDA needs two values of each grouping, a case in every combination, more cases than combinations, and a number that is not constant.
          'if (nlevels(A) < 2 || nlevels(B) < 2 || any(table(A, B) == 0) || length(y) <= nlevels(A) * nlevels(B) || var(y) == 0) stop("too few cases, or a combination with none")',
          'fit <- lm(y ~ A * B, contrasts = list(A = "contr.sum", B = "contr.sum")); dd <- drop1(fit, scope = ~ A + B + A:B, test = "F"); rss <- dd$RSS[1]',
        ],
        expr: (path) => {
          const m = /^factorial\/effects\/(\d)\/(f|p|partial_eta_squared)$/.exec(path);
          if (!m) return undefined;
          const at = Number(m[1]) + 2;
          return m[2] === 'f' ? `dd[["F value"]][${at}]` : m[2] === 'p' ? `dd[["Pr(>F)"]][${at}]` : `dd[["Sum of Sq"]][${at}] / (dd[["Sum of Sq"]][${at}] + rss)`;
        },
      };
    }
    case 'cronbach_alpha':
      return {
        prep: [
          `m <- cbind(${p.vars.map(asNum).join(', ')})`,
          'm <- m[complete.cases(m), , drop = FALSE]',
          'if (nrow(m) < 3) stop("too few cases"); k <- ncol(m); S <- cov(m); if (!(sum(S) > 0)) stop("the sum of the items has no spread")',
          'al <- k / (k - 1) * (1 - sum(diag(S)) / sum(S)); R <- suppressWarnings(cor(m)); rbar <- mean(R[upper.tri(R)])',
        ],
        expr: fixed({ 'alpha/alpha': 'al', 'alpha/standardized_alpha': 'if (is.finite(rbar)) k * rbar / (1 + (k - 1) * rbar) else NA_real_', 'alpha/mean_inter_item_r': 'if (is.finite(rbar)) rbar else NA_real_' }),
      };
    case 'partial_correlation': {
      const controls = p.vars.slice(2);
      return {
        prep: [
          ...take([['x', asNum(v0)], ['y', asNum(v1)], ...controls.map((c, i): [string, string] => [`z${i + 1}`, asNum(c)])]),
          // PrismQDA needs four cases more than there are controls, and a control that another accounts for (or that is the same in every case) is an error to it.
          `ctrl <- cbind(${controls.map((_, i) => `z${i + 1}`).join(', ')}); if (length(x) < ncol(ctrl) + 4) stop("too few cases"); if (anyNA(coef(lm(x ~ ctrl, tol = 1e-10)))) stop("a control is aliased")`,
          'pc <- partial_cor(x, y, ctrl)',
        ],
        expr: fixed({ 'partial/r': 'pc$partial[["r"]]', 'partial/p': 'pc$partial[["p"]]' }),
      };
    }
    case 'linear_regression': {
      const { lines, last } = modelSetup(p, false, true);
      return {
        prep: [...lines, `if (nrow(m) <= length(coef(${last})) || var(m[["y"]]) == 0 || anyNA(coef(${last}))) stop("too few cases, no variation in the outcome, or a predictor is aliased")`, `s <- summary(${last}); at <- function(i, j) if (i <= nrow(s$coefficients)) s$coefficients[i, j] else NA_real_`],
        expr: (path) => {
          const m = /^models\/\d+\/(.+)$/.exec(path);
          if (!m) return undefined;
          const rest = m[1];
          if (rest === 'r_squared') return 's$r.squared';
          if (rest === 'adj_r_squared') return 's$adj.r.squared';
          if (rest === 'f/f') return 'unname(s$fstatistic[1])';
          if (rest === 'f/p') return 'pf(s$fstatistic[1], s$fstatistic[2], s$fstatistic[3], lower.tail = FALSE)[[1]]';
          const t = /^terms\/(\d+)\/(estimate|p)$/.exec(rest);
          if (t) return `at(${Number(t[1]) + 1}, ${t[2] === 'estimate' ? 1 : 4})`;
          return undefined;
        },
      };
    }
    case 'logistic_regression': {
      const { lines, last, count } = modelSetup(p, true);
      return {
        prep: [
          ...lines,
          // A resample the engine cannot fit (the outcome is separated, or the fit does not settle) in any step is left out; the numbers are
          // those of the fit of the last step, made the way PrismQDA makes it.
          `lg <- lapply(list(${Array.from({ length: count }, (_, i) => `fit${i + 1}`).join(', ')}), function(f) pq_logit(model.matrix(f), f$y))`,
          'if (any(vapply(lg, is.null, TRUE))) stop("no fit")',
          'lg <- lg[[length(lg)]]; yy <- fit' + count + '$y; n <- length(yy); ev <- sum(yy == 1); null_dev <- -2 * (ev * log(ev / n) + (n - ev) * log(1 - ev / n))',
          'rk <- rank(lg$mu); at <- function(i) if (i <= length(lg$coef)) lg$coef[i] else NA_real_; se_at <- function(i) if (i <= length(lg$coef)) lg$se[i] else NA_real_',
        ],
        expr: (path) => {
          const m = /^models\/\d+\/(.+)$/.exec(path);
          if (!m) return undefined;
          const rest = m[1];
          if (rest === 'lr/chi2') return '(null_dev - lg$deviance)';
          if (rest === 'lr/p') return 'pchisq(null_dev - lg$deviance, length(lg$coef) - 1, lower.tail = FALSE)';
          if (rest === 'nagelkerke') return '(1 - exp((null_dev - lg$deviance) / -n)) / (1 - exp(-null_dev / n))';
          if (rest === 'auc') return '(sum(rk[yy == 1]) - ev * (ev + 1) / 2) / (ev * (n - ev))';
          const t = /^terms\/(\d+)\/(estimate|odds_ratio|p)$/.exec(rest);
          if (t) {
            const at = Number(t[1]) + 1;
            return t[2] === 'p' ? `2 * pnorm(-abs(at(${at}) / se_at(${at})))` : t[2] === 'estimate' ? `at(${at})` : `exp(at(${at}))`;
          }
          return undefined;
        },
      };
    }
    case 'curve_fit': {
      const formulas: Record<string, string> = {
        linear: 'y ~ x',
        logarithmic: 'y ~ log(x)',
        inverse: 'y ~ I(1 / x)',
        quadratic: 'y ~ x + I(x^2)',
        cubic: 'y ~ x + I(x^2) + I(x^3)',
        power: 'log(y) ~ log(x)',
        exponential: 'log(y) ~ x',
        s_curve: 'log(y) ~ I(1 / x)',
      };
      return {
        prep: [...take([['y', asNum(v0)], ['x', asNum(v1)]]), 'dt <- data.frame(x = x, y = y)'],
        expr: (_path, key) => {
          const f = key ? formulas[key] : undefined;
          if (!f) return undefined;
          const logged = key === 'power' || key === 'exponential' || key === 's_curve';
          return `tryCatch(curve_original(lm(${f}, data = dt), y, ${logged ? 'TRUE' : 'FALSE'})[["r_squared_original"]], error = function(e) NA_real_)`;
        },
      };
    }
    default:
      return undefined;
  }
}

/** The bootstrap of a whole analysis, written as a call that works out the page's numbers again on every resample. Empty when the page has none. */
export function wholeCode(p: RPage): string[] {
  const b = p.options.bootstrap;
  if (!b || !(b.analysis || b.cluster)) return [];
  const rows = p.bootRows ?? [];
  const h = headline(p);
  if (!h || rows.length === 0) return [];
  const exprs = rows.map((r) => h.expr(r.path, r.key) ?? 'NA_real_');
  const body = [...h.prep, `c(${exprs.join(', ')})`].map((l) => `  ${l}`).join('\n');
  const cluster = b.cluster && p.clusterLabel !== undefined ? `, cluster = val(${col({ key: '', label: p.clusterLabel, numeric: false, categories: [] }).replace('d[[', 'd0[[')})` : '';
  return [`pq_boot_whole(d0, function(d) {\n${body}\n}, ${vec(rows.map((r) => r.label))}, c(${rows.map((r) => (r.p ? 'TRUE' : 'FALSE')).join(', ')}), ${plan(p)}${cluster})`];
}
