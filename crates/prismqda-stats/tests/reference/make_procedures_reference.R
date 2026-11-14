# Reference values for the statistical procedures, computed by base R (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_procedures_reference.R
# It writes procedures_reference.json next to this script. tests/procedures_reference.rs requires PrismQDA's procedures
# to match these values.
#
# What is taken from an R function and what is written out in plain R:
#   t tests            stats::t.test (statistic, df, p, interval); Cohen's d, Hedges' g and the approximate intervals of
#                      d are written out from the definitions (Hedges and Olkin, 1985)
#   one-way ANOVA      stats::anova(lm()), stats::oneway.test (Welch), stats::pairwise.t.test (pooled SD); Levene's
#                      test (median centred) as an ANOVA of the absolute deviations from the group medians
#   chi-square         stats::chisq.test (statistic, p, expected, adjusted residuals, Yates); G squared, phi,
#                      Cramer's V, and the contingency coefficient are written out
#   Fisher             stats::fisher.test (p, conditional odds ratio, interval)
#   correlation        stats::cor.test for Pearson, Spearman (exact = FALSE), and Kendall
#   rank tests         stats::wilcox.test (two samples, paired, one sample), stats::kruskal.test; Dunn's comparisons
#                      are written out; p.adjust for the adjustments
#
# The data are synthetic and invented here. Nothing comes from a real study. Some are rounded so that they have ties and
# zero differences, because those are the cases where implementations differ.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}

zc <- qnorm(0.975)
hedges_j <- function(df) exp(lgamma(df / 2) - lgamma((df - 1) / 2)) / sqrt(df / 2)
num <- function(v) as.numeric(unname(v))

# ------------------------------------------------------------------ data
set.seed(101)
d_a <- round(rnorm(14, 50, 8), 1)
d_b <- round(rnorm(17, 56, 15), 1)
d_out <- c(round(rnorm(24, 30, 4), 1), 58.2)                  # one far value, so there is an outlier
d_ints_a <- round(rnorm(22, 20, 5))                           # whole numbers: many ties
d_ints_b <- round(rnorm(26, 23, 6))
d_small_a <- round(rnorm(8, 10, 2), 2)                        # no ties, small: exact Mann-Whitney
d_small_b <- round(rnorm(9, 12, 3), 2)
d_big_a <- round(rnorm(60, 100, 15), 3)                       # 60 and 55 values: normal approximation
d_big_b <- round(rnorm(55, 104, 15), 3)
pre <- round(rnorm(20, 50, 10))                               # paired, whole numbers: ties and zero differences
post <- pre + sample(c(-3:6), 20, replace = TRUE)
pre2 <- round(rnorm(15, 30, 6), 2)                            # paired without ties: exact Wilcoxon
post2 <- round(pre2 + rnorm(15, 1.5, 3), 2)
one_x <- round(rnorm(25, 52, 9), 1)
g1 <- round(rnorm(12, 10, 2), 1); g2 <- round(rnorm(15, 12, 4), 1); g3 <- round(rnorm(9, 15, 3), 1)
h1 <- round(rnorm(10, 5, 1)); h2 <- round(rnorm(10, 6, 1)); h3 <- round(rnorm(10, 7, 2)); h4 <- round(rnorm(10, 5.5, 1))
cx <- round(rnorm(30, 10, 3), 2); cy <- round(0.6 * cx + rnorm(30, 0, 2.5), 2)
ix <- round(rnorm(40, 10, 3)); iy <- round(0.5 * ix + rnorm(40, 0, 3))
kx <- round(rnorm(12, 0, 1), 3); ky <- round(0.5 * kx + rnorm(12, 0, 1), 3)
bx <- round(rnorm(60, 0, 1), 3); by <- round(0.4 * bx + rnorm(60, 0, 1), 3)

# ------------------------------------------------------------------ descriptives
desc <- function(name, x) {
  n <- length(x); m <- mean(x); s <- sd(x)
  q <- quantile(x, c(.25, .5, .75), type = 7); ci <- t.test(x)$conf.int
  g1 <- n / ((n - 1) * (n - 2)) * sum(((x - m) / s)^3)
  g2 <- n * (n + 1) / ((n - 1) * (n - 2) * (n - 3)) * sum(((x - m) / s)^4) - 3 * (n - 1)^2 / ((n - 2) * (n - 3))
  se_g1 <- sqrt(6 * n * (n - 1) / ((n - 2) * (n + 1) * (n + 3)))
  se_g2 <- 2 * se_g1 * sqrt((n^2 - 1) / ((n - 3) * (n + 5)))
  iqr <- num(q[3] - q[1])
  list(name = name, x = x, expected = list(
    n = n, sum = sum(x), mean = m, variance = var(x), sd = s, se = s / sqrt(n), ci_lo = num(ci[1]), ci_hi = num(ci[2]),
    min = min(x), q1 = num(q[1]), median = num(q[2]), q3 = num(q[3]), max = max(x), iqr = iqr,
    skewness = g1, se_skewness = se_g1, kurtosis = g2, se_kurtosis = se_g2,
    outliers = sum(x < q[1] - 1.5 * iqr | x > q[3] + 1.5 * iqr)))
}

# The exact interval of Cohen's d: solve for the noncentrality parameter of the t distribution (Steiger and Fouladi, 1997),
# then multiply by the scale that turns a noncentrality parameter into a d (1 / sqrt(n), or sqrt(1/n1 + 1/n2)).
d_exact <- function(d, scale, df, level = 0.95) {
  tt <- d / scale; a <- (1 - level) / 2; lim <- max(40, abs(tt) + 40)
  lo <- uniroot(function(ncp) pt(tt, df, ncp) - (1 - a), c(-lim, lim), tol = 1e-12)$root
  hi <- uniroot(function(ncp) pt(tt, df, ncp) - a, c(-lim, lim), tol = 1e-12)$root
  c(lo, hi) * scale
}

# ------------------------------------------------------------------ t tests
two_sample <- function(name, x, y, var_equal) {
  r <- t.test(x, y, var.equal = var_equal)
  n1 <- length(x); n2 <- length(y)
  sp <- sqrt(((n1 - 1) * var(x) + (n2 - 1) * var(y)) / (n1 + n2 - 2))
  d <- (mean(x) - mean(y)) / sp
  se_d <- sqrt((n1 + n2) / (n1 * n2) + d^2 / (2 * (n1 + n2)))
  j <- hedges_j(n1 + n2 - 2)
  di <- d_exact(d, sqrt(1 / n1 + 1 / n2), n1 + n2 - 2)
  list(name = name, kind = if (var_equal) "student" else "welch", x = x, y = y, expected = list(
    t = num(r$statistic), df = num(r$parameter), p = r$p.value, estimate = mean(x) - mean(y), ci_lo = num(r$conf.int[1]), ci_hi = num(r$conf.int[2]),
    se = num(r$stderr), d = d, d_lo = di[1], d_hi = di[2], g = d * j, g_lo = di[1] * j, g_hi = di[2] * j))
}
one_sample <- function(name, x, mu) {
  r <- t.test(x, mu = mu); n <- length(x)
  d <- (mean(x) - mu) / sd(x); se_d <- sqrt(1 / n + d^2 / (2 * n)); j <- hedges_j(n - 1); di <- d_exact(d, 1 / sqrt(n), n - 1)
  list(name = name, kind = "one_sample", x = x, mu = mu, expected = list(
    t = num(r$statistic), df = num(r$parameter), p = r$p.value, estimate = mean(x), ci_lo = num(r$conf.int[1]), ci_hi = num(r$conf.int[2]),
    se = num(r$stderr), d = d, d_lo = di[1], d_hi = di[2], g = d * j, g_lo = di[1] * j, g_hi = di[2] * j))
}
paired <- function(name, x, y) {
  r <- t.test(x, y, paired = TRUE); dif <- x - y; n <- length(dif)
  d <- mean(dif) / sd(dif); se_d <- sqrt(1 / n + d^2 / (2 * n)); j <- hedges_j(n - 1); di <- d_exact(d, 1 / sqrt(n), n - 1)
  list(name = name, kind = "paired", x = x, y = y, expected = list(
    t = num(r$statistic), df = num(r$parameter), p = r$p.value, estimate = mean(dif), ci_lo = num(r$conf.int[1]), ci_hi = num(r$conf.int[2]),
    se = num(r$stderr), d = d, d_lo = di[1], d_hi = di[2], g = d * j, g_lo = di[1] * j, g_hi = di[2] * j))
}

# ------------------------------------------------------------------ one-way ANOVA
anova_case <- function(name, groups) {
  y <- unlist(groups); g <- factor(rep(seq_along(groups), lengths(groups)))
  a <- anova(lm(y ~ g)); k <- length(groups)
  ssb <- a[["Sum Sq"]][1]; ssw <- a[["Sum Sq"]][2]; dfb <- a$Df[1]; dfw <- a$Df[2]; msw <- ssw / dfw
  w <- oneway.test(y ~ g)
  med <- tapply(y, g, median); dev <- abs(y - med[g]); lv <- anova(lm(dev ~ g))
  mean_g <- tapply(y, g, mean); n_g <- tapply(y, g, length)
  pw <- function(method) {
    m <- pairwise.t.test(y, g, p.adjust.method = method, pool.sd = TRUE)$p.value
    unlist(lapply(1:(k - 1), function(a1) lapply((a1 + 1):k, function(b1) m[b1 - 1, a1])))
  }
  pairs <- do.call(rbind, lapply(1:(k - 1), function(a1) do.call(rbind, lapply((a1 + 1):k, function(b1) c(a1, b1)))))
  m_pairs <- nrow(pairs)
  diffs <- mean_g[pairs[, 1]] - mean_g[pairs[, 2]]
  ses <- sqrt(msw * (1 / n_g[pairs[, 1]] + 1 / n_g[pairs[, 2]]))
  qb <- qt(1 - 0.05 / (2 * m_pairs), dfw); q1 <- qt(0.975, dfw)
  list(name = name, groups = unname(groups), expected = list(
    ss_between = ssb, ss_within = ssw, df_between = dfb, df_within = dfw, f = a[["F value"]][1], p = a[["Pr(>F)"]][1],
    eta_squared = ssb / (ssb + ssw), omega_squared = (ssb - dfb * msw) / (ssb + ssw + msw),
    welch_f = num(w$statistic), welch_df1 = num(w$parameter[1]), welch_df2 = num(w$parameter[2]), welch_p = w$p.value,
    levene_f = lv[["F value"]][1], levene_df1 = lv$Df[1], levene_df2 = lv$Df[2], levene_p = lv[["Pr(>F)"]][1],
    pairs = unname(lapply(seq_len(m_pairs), function(i) list(a = pairs[i, 1] - 1, b = pairs[i, 2] - 1, difference = num(diffs[i]), se = num(ses[i]), t = num(diffs[i] / ses[i]),
      ci_none_lo = num(diffs[i] - q1 * ses[i]), ci_none_hi = num(diffs[i] + q1 * ses[i]), ci_bonf_lo = num(diffs[i] - qb * ses[i]), ci_bonf_hi = num(diffs[i] + qb * ses[i])))),
    p_none = num(pw("none")), p_bonferroni = num(pw("bonferroni")), p_holm = num(pw("holm")), p_bh = num(pw("BH"))))
}

# ------------------------------------------------------------------ tables of counts
chi_case <- function(name, tab) {
  x <- suppressWarnings(chisq.test(tab, correct = FALSE)); n <- sum(tab); r <- nrow(tab); cc <- ncol(tab); e <- x$expected
  chi2 <- num(x$statistic); df <- num(x$parameter)
  g2 <- 2 * sum(ifelse(tab > 0, tab * log(tab / e), 0))
  out <- list(chi2 = chi2, df = df, p = x$p.value, expected = unname(e), adjusted_residuals = unname(x$stdres), standardized_residuals = unname((tab - e) / sqrt(e)),
    g2 = g2, g2_p = pchisq(g2, df, lower.tail = FALSE), cramers_v = sqrt(chi2 / (n * min(r - 1, cc - 1))), contingency_c = sqrt(chi2 / (chi2 + n)),
    smallest_expected = min(e), share_expected_below_5 = mean(e < 5))
  if (r == 2 && cc == 2) {
    y <- suppressWarnings(chisq.test(tab, correct = TRUE))
    out$yates_chi2 <- num(y$statistic); out$yates_p <- y$p.value
    out$phi <- (tab[1, 1] * tab[2, 2] - tab[1, 2] * tab[2, 1]) / sqrt(prod(rowSums(tab)) * prod(colSums(tab)))
  }
  list(name = name, table = unname(tab), expected = out)
}
# fisher.test finds the odds ratio and its interval with uniroot at R's default tolerance (about 1.2e-4), which is only
# good to a few thousandths when the odds ratio is large. This repeats its method (the conditional distribution of the
# first cell given the margins) with the roots found to 1e-14, so PrismQDA can be compared to the exact answer as well.
fisher_precise <- function(a, b, c, d, conf.level = 0.95) {
  m <- a + c; n <- b + d; k <- a + b; x <- a
  lo <- max(0, k - n); hi <- min(k, m); support <- lo:hi
  logdc <- dhyper(support, m, n, k, log = TRUE)
  dnhyper <- function(ncp) { d <- logdc + log(ncp) * support; d <- exp(d - max(d)); d / sum(d) }
  mnhyper <- function(ncp) { if (ncp == 0) return(lo); if (ncp == Inf) return(hi); sum(support * dnhyper(ncp)) }
  pnhyper <- function(q, ncp = 1, upper.tail = FALSE) {
    if (ncp == 1) return(if (upper.tail) phyper(x - 1, m, n, k, lower.tail = FALSE) else phyper(x, m, n, k))
    if (ncp == 0) return(as.numeric(if (upper.tail) q <= lo else q >= lo))
    if (ncp == Inf) return(as.numeric(if (upper.tail) q <= hi else q >= hi))
    sum(dnhyper(ncp)[if (upper.tail) support >= q else support <= q])
  }
  tol <- 1e-14; eps <- .Machine$double.eps
  mle <- if (x == lo) 0 else if (x == hi) Inf else {
    mu <- mnhyper(1)
    if (mu > x) uniroot(function(t) mnhyper(t) - x, c(0, 1), tol = tol)$root
    else if (mu < x) 1 / uniroot(function(t) mnhyper(1 / t) - x, c(eps, 1), tol = tol)$root else 1
  }
  alpha <- (1 - conf.level) / 2
  ncp_u <- if (x == hi) Inf else {
    p <- pnhyper(x, 1)
    if (p < alpha) uniroot(function(t) pnhyper(x, t) - alpha, c(0, 1), tol = tol)$root
    else if (p > alpha) 1 / uniroot(function(t) pnhyper(x, 1 / t) - alpha, c(eps, 1), tol = tol)$root else 1
  }
  ncp_l <- if (x == lo) 0 else {
    p <- pnhyper(x, 1, upper.tail = TRUE)
    if (p > alpha) uniroot(function(t) pnhyper(x, t, upper.tail = TRUE) - alpha, c(0, 1), tol = tol)$root
    else if (p < alpha) 1 / uniroot(function(t) pnhyper(x, 1 / t, upper.tail = TRUE) - alpha, c(eps, 1), tol = tol)$root else 1
  }
  list(odds_ratio_precise = mle, ci_lo_precise = ncp_l, ci_hi_precise = ncp_u)
}
fisher_case <- function(name, a, b, c, d) {
  f <- fisher.test(matrix(c(a, c, b, d), 2))
  list(name = name, table = c(a, b, c, d), expected = c(list(p = f$p.value, odds_ratio = num(f$estimate), ci_lo = f$conf.int[1], ci_hi = f$conf.int[2]), fisher_precise(a, b, c, d)))
}

# ------------------------------------------------------------------ correlation
cor_case <- function(name, x, y) {
  pe <- cor.test(x, y); sp <- suppressWarnings(cor.test(x, y, method = "spearman")); ke <- suppressWarnings(cor.test(x, y, method = "kendall"))
  list(name = name, x = x, y = y, expected = list(
    pearson_r = num(pe$estimate), pearson_t = num(pe$statistic), pearson_df = num(pe$parameter), pearson_p = pe$p.value, pearson_ci_lo = pe$conf.int[1], pearson_ci_hi = pe$conf.int[2],
    spearman_rho = num(sp$estimate), spearman_p = sp$p.value,
    kendall_tau = num(ke$estimate), kendall_p = ke$p.value, kendall_exact = names(ke$statistic) == "T", kendall_z = if (names(ke$statistic) == "z") num(ke$statistic) else NULL))
}

# ------------------------------------------------------------------ rank tests
mw_case <- function(name, x, y) {
  w <- suppressWarnings(wilcox.test(x, y)); n1 <- length(x); n2 <- length(y); r <- rank(c(x, y))
  list(name = name, x = x, y = y, expected = list(
    w = num(w$statistic), p = w$p.value, exact = (n1 < 50 && n2 < 50 && !anyDuplicated(c(x, y))),
    mean_rank_1 = mean(r[1:n1]), mean_rank_2 = mean(r[(n1 + 1):(n1 + n2)]), rank_biserial = 2 * num(w$statistic) / (n1 * n2) - 1))
}
signed_case <- function(name, x, y = NULL, mu = 0) {
  w <- if (is.null(y)) suppressWarnings(wilcox.test(x, mu = mu)) else suppressWarnings(wilcox.test(x, y, paired = TRUE))
  d <- if (is.null(y)) x - mu else x - y
  nz <- d[d != 0]; r <- rank(abs(nz)); v <- sum(r[nz > 0]); total <- length(nz) * (length(nz) + 1) / 2
  list(name = name, x = x, y = y, mu = mu, expected = list(
    v = num(w$statistic), p = w$p.value, n = length(nz), zeros = sum(d == 0),
    exact = (length(nz) < 50 && !anyDuplicated(abs(nz)) && all(d != 0)), rank_biserial = (2 * v - total) / total))
}
kw_case <- function(name, groups) {
  y <- unlist(groups); g <- factor(rep(seq_along(groups), lengths(groups))); k <- length(groups); N <- length(y)
  kt <- kruskal.test(y ~ g); r <- rank(y); mr <- tapply(r, g, mean); ng <- tapply(y, g, length)
  tt <- table(y); base <- N * (N + 1) / 12 - sum(tt^3 - tt) / (12 * (N - 1))
  pairs <- do.call(rbind, lapply(1:(k - 1), function(a) do.call(rbind, lapply((a + 1):k, function(b) c(a, b)))))
  z <- (mr[pairs[, 1]] - mr[pairs[, 2]]) / sqrt(base * (1 / ng[pairs[, 1]] + 1 / ng[pairs[, 2]]))
  p <- 2 * pnorm(-abs(z))
  list(name = name, groups = unname(groups), expected = list(
    h = num(kt$statistic), df = num(kt$parameter), p = kt$p.value, epsilon_squared = num(kt$statistic) / (N - 1), eta_squared_h = (num(kt$statistic) - k + 1) / (N - k),
    mean_ranks = num(mr), dunn_z = num(z), dunn_p = num(p), dunn_p_holm = num(p.adjust(p, "holm")), dunn_p_bonferroni = num(p.adjust(p, "bonferroni"))))
}

pvals <- c(0.001, 0.008, 0.039, 0.041, 0.042, 0.06, 0.074, 0.205, 0.212, 0.216, 0.222, 0.251, 0.269, 0.275, 0.34, 0.341, 0.384, 0.569, 0.594, 0.696, 0.762, 0.94, 0.942, 0.975, 0.986)

result <- list(
  descriptives = list(desc("two_groups_a", d_a), desc("with_an_outlier", d_out), desc("whole_numbers", d_ints_a), desc("small", d_small_a)),
  ttests = list(
    two_sample("welch_unequal_variances", d_a, d_b, FALSE), two_sample("student_unequal_variances", d_a, d_b, TRUE),
    two_sample("welch_whole_numbers", d_ints_a, d_ints_b, FALSE), two_sample("student_whole_numbers", d_ints_a, d_ints_b, TRUE),
    one_sample("one_sample", one_x, 48), one_sample("one_sample_whole", d_ints_a, 20),
    paired("paired_whole_numbers", pre, post), paired("paired_continuous", pre2, post2)),
  anova = list(anova_case("three_groups_unequal", list(g1, g2, g3)), anova_case("four_groups_equal", list(h1, h2, h3, h4))),
  chi_square = list(
    chi_case("two_by_two", matrix(c(12, 7, 5, 14), 2)), chi_case("two_by_two_small", matrix(c(3, 1, 1, 3), 2)),
    chi_case("three_by_four", matrix(c(20, 15, 9, 11, 18, 14, 7, 12, 6, 14, 22, 10), 3)), chi_case("two_by_three", matrix(c(30, 18, 12, 25, 22, 20), 2)),
    chi_case("with_a_zero_cell", matrix(c(10, 0, 5, 8, 6, 9), 2))),
  fisher = list(fisher_case("balanced", 12, 5, 7, 14), fisher_case("small", 3, 1, 1, 3), fisher_case("zero_cell", 10, 0, 2, 8), fisher_case("rare", 1, 9, 11, 3), fisher_case("lopsided", 2, 18, 5, 40), fisher_case("empty_corner_low", 0, 6, 5, 4)),
  correlation = list(cor_case("continuous", cx, cy), cor_case("whole_numbers_with_ties", ix, iy), cor_case("small_exact_kendall", kx, ky), cor_case("sixty_pairs", bx, by)),
  mann_whitney = list(mw_case("exact_no_ties", d_small_a, d_small_b), mw_case("ties", d_ints_a, d_ints_b), mw_case("large_no_ties", d_big_a, d_big_b), mw_case("continuous_unequal", d_a, d_b)),
  wilcoxon = list(signed_case("paired_with_ties_and_zeros", pre, post), signed_case("paired_exact", pre2, post2), signed_case("one_sample", one_x, NULL, 50), signed_case("one_sample_whole_numbers", d_ints_a, NULL, 19)),
  kruskal = list(kw_case("three_groups_continuous", list(g1, g2, g3)), kw_case("four_groups_ties", list(h1, h2, h3, h4))),
  p_adjust = list(p = pvals, none = pvals, bonferroni = p.adjust(pvals, "bonferroni"), holm = p.adjust(pvals, "holm"), bh = p.adjust(pvals, "BH"))
)

path <- file.path(script_dir(), "procedures_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE, null = "null"), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
