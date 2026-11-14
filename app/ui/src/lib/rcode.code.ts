// R code for the analyses of the Statistics page: what a recorded script runs. Nothing in this file is words for the person, so it is
// not audited for language (see scripts/i18n/ast.mjs); the words of a script (its comments) are written in rScript.ts.
//
// Each analysis is written with the R function the engine is compared with (t.test, aov, chisq.test, cor.test, wilcox.test, ...), so the
// script prints the numbers the page shows. Where base R has no function (Games-Howell, Dunn, the runs test, the Moses test, Somers' d,
// partial correlation, ...) the definition is written out in a helper, the same one the validation of the engine is made with.

import type { StatOptions } from '../types';
import { BOOT_HELPERS, bootCode, wholeCode } from './rboot.code';
import { CHART_HELPERS, chartCode } from './rchart.code';
import { ADJUST, asNum, asVal, grouped, level, modelSetup, q, rn, take, vec, type Helper, type RPage, type RVar } from './rbase.code';

export { q, rn };
export type { RPage, RVar };

// ---------------------------------------------------------------- the helper functions a script defines


const BASE_HELPERS: Record<string, Helper> = {
  num: { code: 'num <- function(x) suppressWarnings(as.numeric(x))' },
  val: {
    code: `val <- function(x) {
  if (!is.numeric(x)) return(as.character(x))
  vapply(x, function(v) if (is.na(v)) NA_character_ else format(v, scientific = FALSE, trim = TRUE, digits = 15), "")
}`,
  },
  skew: { code: 'skew <- function(x) { n <- length(x); n / ((n - 1) * (n - 2)) * sum(((x - mean(x)) / sd(x))^3) }' },
  kurt: {
    code: `kurt <- function(x) {
  n <- length(x)
  n * (n + 1) / ((n - 1) * (n - 2) * (n - 3)) * sum(((x - mean(x)) / sd(x))^4) - 3 * (n - 1)^2 / ((n - 2) * (n - 3))
}`,
  },
  hedges_j: { code: 'hedges_j <- function(df) exp(lgamma(df / 2) - lgamma((df - 1) / 2)) / sqrt(df / 2)' },
  d_ci: {
    code: `d_ci <- function(d, scale, df, level = 0.95) {
  tt <- d / scale; a <- (1 - level) / 2; lim <- max(40, abs(tt) + 40)
  lo <- suppressWarnings(uniroot(function(ncp) pt(tt, df, ncp) - (1 - a), c(-lim, lim), tol = 1e-12))$root
  hi <- suppressWarnings(uniroot(function(ncp) pt(tt, df, ncp) - a, c(-lim, lim), tol = 1e-12))$root
  c(lower = lo * scale, upper = hi * scale)
}`,
  },
  levene: {
    code: `levene <- function(y, g) {
  med <- tapply(y, g, median); dev <- abs(y - med[as.integer(g)]); a <- anova(lm(dev ~ g))
  c(F = a[["F value"]][1], df1 = a$Df[1], df2 = a$Df[2], p = a[["Pr(>F)"]][1])
}`,
  },
  posthoc_pairs: {
    code: `posthoc_pairs <- function(y, g, method = "holm", level = 0.95) {
  k <- nlevels(g); n <- tapply(y, g, length); m <- tapply(y, g, mean); v <- tapply(y, g, var); N <- length(y)
  dfw <- N - k; mse <- sum((y - m[as.integer(g)])^2) / dfw
  pairs <- t(combn(k, 2)); out <- NULL
  for (r in seq_len(nrow(pairs))) {
    i <- pairs[r, 1]; j <- pairs[r, 2]; diff <- unname(m[i] - m[j])
    se <- sqrt(mse * (1 / n[i] + 1 / n[j])); tt <- diff / se; p_t <- 2 * pt(-abs(tt), dfw); df <- dfw; p <- p_t
    if (method == "tukey") {
      p <- ptukey(abs(tt) * sqrt(2), k, dfw, lower.tail = FALSE); crit <- qtukey(level, k, dfw) / sqrt(2)
    } else if (method == "games_howell") {
      se <- sqrt(v[i] / n[i] + v[j] / n[j]); df <- (v[i] / n[i] + v[j] / n[j])^2 / ((v[i] / n[i])^2 / (n[i] - 1) + (v[j] / n[j])^2 / (n[j] - 1))
      tt <- diff / se; p <- ptukey(abs(tt) * sqrt(2), k, df, lower.tail = FALSE); crit <- qtukey(level, k, df) / sqrt(2)
    } else if (method == "scheffe") {
      f <- diff^2 / ((k - 1) * se^2); p <- pf(f, k - 1, dfw, lower.tail = FALSE); crit <- sqrt((k - 1) * qf(level, k - 1, dfw))
    } else {
      alpha <- if (method == "bonferroni") (1 - level) / choose(k, 2) else 1 - level; crit <- qt(1 - alpha / 2, dfw)
    }
    out <- rbind(out, data.frame(comparison = paste(levels(g)[i], levels(g)[j], sep = " - "), difference = diff, lower = diff - crit * se, upper = diff + crit * se,
                                 t = unname(tt), df = unname(df), p = unname(p_t), test_p = unname(p), row.names = NULL))
  }
  if (!method %in% c("tukey", "games_howell", "scheffe")) out$adjusted_p <- p.adjust(out$p, method)
  out
}`,
  },
  dunn_pairs: {
    code: `dunn_pairs <- function(y, g, method = "holm") {
  N <- length(y); k <- nlevels(g); r <- rank(y); mr <- tapply(r, g, mean); ng <- tapply(y, g, length)
  tt <- table(y); base <- N * (N + 1) / 12 - sum(tt^3 - tt) / (12 * (N - 1))
  pairs <- t(combn(k, 2)); z <- (mr[pairs[, 1]] - mr[pairs[, 2]]) / sqrt(base * (1 / ng[pairs[, 1]] + 1 / ng[pairs[, 2]]))
  p <- 2 * pnorm(-abs(z))
  data.frame(comparison = paste(levels(g)[pairs[, 1]], levels(g)[pairs[, 2]], sep = " - "), rank_difference = unname(mr[pairs[, 1]] - mr[pairs[, 2]]), z = unname(z), p = unname(p), adjusted_p = p.adjust(p, method), row.names = NULL)
}`,
  },
  signed_rank: {
    code: `signed_rank <- function(dif) {
  nz <- dif[dif != 0]; n <- length(nz); v <- sum(rank(abs(nz))[nz > 0]); total <- n * (n + 1) / 2
  ties <- table(abs(nz)); z <- v - n * (n + 1) / 4
  sigma <- sqrt(n * (n + 1) * (2 * n + 1) / 24 - sum(ties^3 - ties) / 48)
  c(pairs_used = n, equal_pairs = sum(dif == 0), V = v, z_normal_approximation = (z - sign(z) * 0.5) / sigma, rank_biserial = (2 * v - total) / total)
}`,
  },
  fisher_precise: {
    code: `fisher_precise <- function(tab, level = 0.95) {
  a <- tab[1, 1]; b <- tab[1, 2]; c <- tab[2, 1]; d <- tab[2, 2]
  m <- a + c; n <- b + d; k <- a + b; x <- a; lo <- max(0, k - n); hi <- min(k, m); support <- lo:hi
  logdc <- dhyper(support, m, n, k, log = TRUE)
  dnhyper <- function(ncp) { w <- logdc + log(ncp) * support; w <- exp(w - max(w)); w / sum(w) }
  mnhyper <- function(ncp) { if (ncp == 0) return(lo); if (ncp == Inf) return(hi); sum(support * dnhyper(ncp)) }
  pnhyper <- function(q, ncp = 1, upper.tail = FALSE) {
    if (ncp == 1) return(if (upper.tail) phyper(x - 1, m, n, k, lower.tail = FALSE) else phyper(x, m, n, k))
    if (ncp == 0) return(as.numeric(if (upper.tail) q <= lo else q >= lo))
    if (ncp == Inf) return(as.numeric(if (upper.tail) q <= hi else q >= hi))
    sum(dnhyper(ncp)[if (upper.tail) support >= q else support <= q])
  }
  tol <- 1e-14; eps <- .Machine$double.eps; alpha <- (1 - level) / 2
  mle <- if (x == lo) 0 else if (x == hi) Inf else {
    mu <- mnhyper(1)
    if (mu > x) uniroot(function(s) mnhyper(s) - x, c(0, 1), tol = tol)$root
    else if (mu < x) 1 / uniroot(function(s) mnhyper(1 / s) - x, c(eps, 1), tol = tol)$root else 1
  }
  upper <- if (x == hi) Inf else {
    p <- pnhyper(x, 1)
    if (p < alpha) uniroot(function(s) pnhyper(x, s) - alpha, c(0, 1), tol = tol)$root
    else if (p > alpha) 1 / uniroot(function(s) pnhyper(x, 1 / s) - alpha, c(eps, 1), tol = tol)$root else 1
  }
  lower <- if (x == lo) 0 else {
    p <- pnhyper(x, 1, upper.tail = TRUE)
    if (p > alpha) uniroot(function(s) pnhyper(x, s, upper.tail = TRUE) - alpha, c(0, 1), tol = tol)$root
    else if (p < alpha) 1 / uniroot(function(s) pnhyper(x, 1 / s, upper.tail = TRUE) - alpha, c(eps, 1), tol = tol)$root else 1
  }
  c(odds_ratio = mle, lower = lower, upper = upper)
}`,
  },
  chi_extras: {
    code: `chi_extras <- function(tab, x) {
  n <- sum(tab); e <- x$expected; chi2 <- unname(x$statistic)
  g2 <- 2 * sum(ifelse(tab > 0, tab * log(tab / e), 0))
  out <- c(cramers_v = sqrt(chi2 / (n * min(dim(tab) - 1))), contingency_c = sqrt(chi2 / (chi2 + n)), G2 = g2, G2_p = pchisq(g2, unname(x$parameter), lower.tail = FALSE))
  if (all(dim(tab) == 2)) out <- c(out, phi = (tab[1, 1] * tab[2, 2] - tab[1, 2] * tab[2, 1]) / sqrt(prod(rowSums(tab)) * prod(colSums(tab))))
  out
}`,
  },
  runs_test: {
    code: `runs_test <- function(s) {
  n1 <- sum(s); n0 <- sum(!s); n <- n1 + n0; nr <- 1 + sum(s[-1] != s[-length(s)])
  mu <- 2 * n1 * n0 / n + 1; sigma <- sqrt(2 * n1 * n0 * (2 * n1 * n0 - n) / (n^2 * (n - 1)))
  cc <- if (nr > mu) -0.5 else if (nr < mu) 0.5 else 0
  z <- (nr - mu + cc) / sigma
  dens <- function(r) {
    if (r %% 2 == 0) { k <- r / 2; 2 * choose(n1 - 1, k - 1) * choose(n0 - 1, k - 1) / choose(n, n1) }
    else { k <- (r - 1) / 2; (choose(n1 - 1, k) * choose(n0 - 1, k - 1) + choose(n1 - 1, k - 1) * choose(n0 - 1, k)) / choose(n, n1) }
  }
  rs <- 2:min(n, 2 * min(n1, n0) + 1); d <- sapply(rs, dens)
  c(runs = nr, upper_part = n1, lower_part = n0, expected = mu, sd = sigma, z = z, p_normal = 2 * pnorm(-abs(z)),
    p_exact = min(1, 2 * min(sum(d[rs <= nr]), sum(d[rs >= nr]))))
}`,
  },
  moses_test: {
    code: `moses_span_p <- function(total, nc, trim, s) {
  kept <- nc - 2 * trim
  if (kept < 2) return(NaN)
  lnc <- function(n, k) ifelse(k > n | k < 0, -Inf, lchoose(n, k))
  top <- min(s, total - 2 * trim)
  if (top < kept) return(0)
  p <- 0
  for (g in kept:top) for (a in 1:(total + 1 - g)) {
    tt <- lnc(a - 1, trim) + lnc(g - 2, kept - 2) + lnc(total + 1 - a - g, trim) - lchoose(total, nc)
    if (is.finite(tt)) p <- p + exp(tt)
  }
  min(p, 1)
}
moses_test <- function(control, test, trim = NULL) {
  nc <- length(control); nt <- length(test); h <- if (is.null(trim)) max(1, floor(0.05 * nc)) else trim
  pooled <- data.frame(v = c(control, test), is_control = c(rep(TRUE, nc), rep(FALSE, nt)))
  pos <- which(pooled[order(pooled$v, !pooled$is_control), "is_control"])
  span <- function(h) pos[nc - h] - pos[h + 1] + 1
  c(span = span(0), p = moses_span_p(nc + nt, nc, 0, span(0)), trim = h, trimmed_span = span(h), trimmed_p = moses_span_p(nc + nt, nc, h, span(h)))
}`,
  },
  median_test: {
    code: `median_test <- function(y, g) {
  m <- median(y); tab <- rbind(above = tapply(y > m, g, sum), at_or_below = tapply(y <= m, g, sum))
  x <- suppressWarnings(chisq.test(tab, correct = FALSE))
  out <- list(median = m, table = tab, chisq = x)
  if (nlevels(g) == 2) out$fisher_p <- fisher.test(tab)$p.value
  out
}`,
  },
  ordinal_assoc: {
    code: `ordinal_assoc <- function(t) {
  r <- nrow(t); k <- ncol(t); n <- sum(t); ri <- rowSums(t); cj <- colSums(t)
  A <- D <- matrix(0, r, k)
  for (i in 1:r) for (j in 1:k) for (u in 1:r) for (v in 1:k) {
    if ((u > i && v > j) || (u < i && v < j)) A[i, j] <- A[i, j] + t[u, v]
    if ((u > i && v < j) || (u < i && v > j)) D[i, j] <- D[i, j] + t[u, v]
  }
  d <- A - D; P <- sum(t * A); Q <- sum(t * D)
  wr <- n^2 - sum(ri^2); wc <- n^2 - sum(cj^2); w <- sqrt(wr * wc); m <- min(r, k)
  s0 <- sqrt(sum(t * d^2) - (P - Q)^2 / n)
  gamma <- (P - Q) / (P + Q); taub <- (P - Q) / w; tauc <- (P - Q) * m / (n^2 * (m - 1))
  v <- outer(ri, rep(1, k)) * wc + outer(rep(1, r), cj) * wr
  ase <- c(4 / (P + Q)^2 * sqrt(sum(t * (Q * A - P * D)^2)), 1 / w^2 * sqrt(sum(t * (2 * w * d + taub * v)^2) - n^3 * taub^2 * (wr + wc)^2),
           2 * m / ((m - 1) * n^2) * s0, 2 / wr^2 * sqrt(sum(t * (wr * d - (P - Q) * (n - outer(ri, rep(1, k))))^2)),
           2 / wc^2 * sqrt(sum(t * (wc * d - (P - Q) * (n - outer(rep(1, r), cj)))^2)))
  est <- c(gamma, taub, tauc, (P - Q) / wr, (P - Q) / wc)
  ase0 <- c(2 / (P + Q) * s0, 2 / w * s0, 2 / ((m - 1) * n^2) * m * s0, 2 / wr * s0, 2 / wc * s0)
  out <- data.frame(measure = c("gamma", "tau-b", "tau-c", "Somers d (columns depend on rows)", "Somers d (rows depend on columns)"), estimate = est, ase = ase,
                    z = est / ase0, p = 2 * pnorm(-abs(est / ase0)))
  attr(out, "pairs") <- c(concordant = P / 2, discordant = Q / 2, somers_d_symmetric = (P - Q) / ((wr + wc) / 2))
  out
}`,
  },
  partial_cor: {
    code: `partial_cor <- function(x, y, controls, level = 0.95) {
  controls <- as.matrix(controls); n <- length(x); k <- ncol(controls)
  r <- cor(resid(lm(x ~ controls)), resid(lm(y ~ controls))); df <- n - 2 - k
  tt <- r * sqrt(df / (1 - r^2)); z <- atanh(r); se <- 1 / sqrt(n - 3 - k)
  list(partial = c(r = r, t = tt, df = df, p = 2 * pt(-abs(tt), df)), interval = tanh(z + c(lower = -1, upper = 1) * qnorm(1 - (1 - level) / 2) * se),
       without_controls = cor.test(x, y, conf.level = level))
}`,
  },
  std_beta: {
    code: `std_beta <- function(fit) {
  x <- model.matrix(fit)[, -1, drop = FALSE]; y <- model.response(model.frame(fit))
  coef(fit)[-1] * apply(x, 2, sd) / sd(y)
}`,
  },
  vif_values: {
    code: `vif_values <- function(fit) {
  x <- model.matrix(fit)[, -1, drop = FALSE]
  if (ncol(x) < 2) return(NULL)
  diag(solve(cor(x)))
}`,
  },
  profile_ci: {
    code: `profile_ci <- function(fit, level = 0.95) {
  X <- model.matrix(fit); y <- fit$y; target <- deviance(fit) + qchisq(level, 1)
  est <- coef(fit); se <- sqrt(diag(vcov(fit)))
  end <- function(j, dir) {
    g <- function(b) tryCatch(suppressWarnings(glm.fit(X[, -j, drop = FALSE], y, offset = b * X[, j], family = binomial(), control = glm.control(epsilon = 1e-14, maxit = 100))$deviance) - target, error = function(e) Inf)
    step <- se[j]
    for (i in 1:40) { far <- est[j] + dir * step; if (g(far) > 0) break; step <- step * 2 }
    uniroot(g, sort(c(est[j], far)), tol = 1e-13, maxiter = 500)$root
  }
  out <- t(sapply(seq_along(est)[-1], function(j) c(end(j, -1), end(j, 1))))
  out <- cbind(out, exp(out)); rownames(out) <- names(est)[-1]
  colnames(out) <- c("B low", "B high", "OR low", "OR high"); out
}`,
  },
  pseudo_r2: {
    code: `pseudo_r2 <- function(fit) {
  n <- nobs(fit); ll <- as.numeric(logLik(fit)); ll0 <- -fit$null.deviance / 2
  c(mcfadden = 1 - ll / ll0, cox_snell = 1 - exp(2 / n * (ll0 - ll)), nagelkerke = (1 - exp(2 / n * (ll0 - ll))) / (1 - exp(2 / n * ll0)))
}`,
  },
  hosmer_lemeshow: {
    code: `hosmer_lemeshow <- function(fit, g = 10) {
  y <- fit$y; yhat <- fitted(fit)
  qq <- unique(quantile(yhat, probs = seq(0, 1, 1 / g)))
  cuts <- cut(yhat, breaks = qq, include.lowest = TRUE)
  observed <- xtabs(cbind(y0 = 1 - y, y1 = y) ~ cuts); expected <- xtabs(cbind(yhat0 = 1 - yhat, yhat1 = yhat) ~ cuts)
  chi2 <- sum((observed - expected)^2 / expected); df <- nrow(expected) - 2
  print(c(chi2 = chi2, df = df, p = pchisq(chi2, df, lower.tail = FALSE)))
  print(cbind(cases = rowSums(observed), observed = observed[, "y1"], expected = expected[, "yhat1"]))
}`,
  },
  classify_auc: {
    code: `classify_auc <- function(fit, cutoff = 0.5) {
  y <- fit$y; p <- fitted(fit)
  tab <- table(observed = factor(y, levels = c(0, 1)), predicted = factor(as.integer(p >= cutoff), levels = c(0, 1))); print(tab)
  print(c(specificity = unname(100 * tab["0", "0"] / sum(tab["0", ])), sensitivity = unname(100 * tab["1", "1"] / sum(tab["1", ])), correct = unname(100 * sum(diag(tab)) / sum(tab))))
  r <- rank(p); n1 <- sum(y == 1); n0 <- sum(y == 0)
  print(c(auc = (sum(r[y == 1]) - n1 * (n1 + 1) / 2) / (n1 * n0)))
}`,
  },
  curve_original: {
    code: `curve_original <- function(fit, y, logged) {
  yhat <- if (logged) exp(fitted(fit)) else fitted(fit)
  c(r_squared_original = 1 - sum((y - yhat)^2) / sum((y - mean(y))^2), rmse_original = sqrt(mean((y - yhat)^2)))
}`,
  },
};

/** The helpers, by name. A script defines only those its pages call, and what they call in turn. */
export const HELPERS: Record<string, Helper> = { ...BASE_HELPERS, ...BOOT_HELPERS, ...CHART_HELPERS };

/** The helpers a piece of script calls, with what they need, each once, in an order in which every one is defined before it is used. */
export function helpersFor(code: string): string {
  const wanted = new Set<string>();
  const add = (name: string) => {
    if (wanted.has(name)) return;
    wanted.add(name);
    for (const need of HELPERS[name].needs ?? []) add(need);
  };
  for (const name of Object.keys(HELPERS)) {
    if (new RegExp(`(^|[^A-Za-z0-9_.])${name}\\(`).test(code)) add(name);
  }
  return Object.keys(HELPERS)
    .filter((n) => wanted.has(n))
    .map((n) => HELPERS[n].code)
    .join('\n');
}

// ---------------------------------------------------------------- the frame of a script

/** The line that announces a page in what R prints. */
export function announce(title: string): string {
  return `cat("\\n", ${q(title)}, "\\n", sep = "")`;
}

/** The line that reads the data: the file, and what each column holds. */
export function readData(classes: Record<string, string>): string {
  const list = Object.entries(classes).map(([name, kind]) => `${q(name)} = ${q(kind)}`);
  return `data <- read.csv("{{data-file}}", check.names = FALSE, na.strings = "", fileEncoding = "UTF-8", colClasses = c(${list.join(', ')}))`;
}

// ---------------------------------------------------------------- one page

/** The effect size of a difference of two means, with its exact interval. */
const meanDifferenceEffect = (lvl: string) => [
  'n1 <- length(a); n2 <- length(b)',
  'sp <- sqrt(((n1 - 1) * var(a) + (n2 - 1) * var(b)) / (n1 + n2 - 2)); dd <- (mean(a) - mean(b)) / sp',
  'print(c(cohens_d = dd, hedges_g = dd * hedges_j(n1 + n2 - 2)))',
  `print(d_ci(dd, sqrt(1 / n1 + 1 / n2), n1 + n2 - 2, ${lvl}))`,
];

/** The R code of one page: the same analysis, on the variables of `data`, printing the numbers the page shows. */
export function pageCode(p: RPage): string {
  const o = p.options;
  const lvl = level(o);
  const L: string[] = [];
  const add = (...lines: string[]) => L.push(...lines);
  const filter = o.filter?.trim();
  add(filter ? `d <- subset(data, ${filter})` : 'd <- data');
  // Counting each case as many times as a weight says is writing the case out that many times.
  const weight = o.weight?.trim();
  if (weight) add(`w <- d[[${q(weight)}]]`, 'd <- d[rep(seq_len(nrow(d)), ifelse(is.na(w), 0, w)), , drop = FALSE]');
  // A bootstrap of the whole analysis goes back to the cases as they were (a curve fit goes on with a table of its own).
  if (o.bootstrap?.analysis || o.bootstrap?.cluster) add('d0 <- d');
  const [v0, v1] = p.vars;

  switch (p.procedure) {
    case 'describe':
      if (p.kind === 'frequencies') {
        add(`x <- ${asVal(v0)}`, `tab <- table(factor(x, levels = ${vec(p.counts ?? v0.categories)}))`);
        add('print(cbind(n = tab, percent = round(100 * tab / length(x), 1), valid_percent = round(100 * tab / sum(tab), 1)))', 'print(c(valid = sum(tab), missing = sum(is.na(x))))');
      } else {
        add(...take([['x', asNum(v0)]]), 'n <- length(x)');
        add('print(c(cases = nrow(d), n = n, missing = nrow(d) - n, mean = mean(x), sd = sd(x), se = sd(x) / sqrt(n)))');
        add(`print(t.test(x, conf.level = ${lvl})$conf.int)`);
        add('print(quantile(x, c(0, .25, .5, .75, 1), type = 7))', 'print(c(skewness = skew(x), kurtosis = kurt(x)))');
      }
      break;
    case 'one_sample_t':
      add(...take([['x', asNum(v0)]]));
      add(`print(t.test(x, mu = ${rn(o.mu ?? 0)}, conf.level = ${lvl}))`);
      add(`dz <- (mean(x) - ${rn(o.mu ?? 0)}) / sd(x)`, 'print(c(cohens_d = dz, hedges_g = dz * hedges_j(length(x) - 1)))', `print(d_ci(dz, 1 / sqrt(length(x)), length(x) - 1, ${lvl}))`);
      break;
    case 'paired_t':
      add(...take([['x', asNum(v0)], ['y', asNum(v1)]]));
      add(`print(t.test(x, y, paired = TRUE, conf.level = ${lvl}))`);
      add('dz <- mean(x - y) / sd(x - y)', 'print(c(cohens_d = dz, hedges_g = dz * hedges_j(length(x) - 1)))', `print(d_ci(dz, 1 / sqrt(length(x)), length(x) - 1, ${lvl}))`);
      break;
    case 'wilcoxon':
      if (p.vars.length === 2) {
        add(...take([['x', asNum(v0)], ['y', asNum(v1)]]));
        add('print(suppressWarnings(wilcox.test(x, y, paired = TRUE)))', 'print(c(median_difference = median(x - y)))', 'print(signed_rank(x - y))');
      } else {
        add(...take([['x', asNum(v0)]]));
        add(`print(suppressWarnings(wilcox.test(x, mu = ${rn(o.mu ?? 0)})))`, 'print(c(median = median(x)))', `print(signed_rank(x - ${rn(o.mu ?? 0)}))`);
      }
      break;
    case 'independent_t':
      add(...grouped(p), 'a <- y[g == levels(g)[1]]; b <- y[g == levels(g)[2]]');
      add(`print(t.test(a, b, var.equal = ${o.equal_variances ? 'TRUE' : 'FALSE'}, conf.level = ${lvl}))`, 'print(c(mean_difference = mean(a) - mean(b)))', ...meanDifferenceEffect(lvl), 'print(levene(y, g))');
      break;
    case 'mann_whitney':
      add(...grouped(p), 'a <- y[g == levels(g)[1]]; b <- y[g == levels(g)[2]]');
      add('w <- suppressWarnings(wilcox.test(a, b)); print(w)', 'print(tapply(rank(y), g, mean))', 'print(c(rank_biserial = 2 * unname(w$statistic) / (length(a) * length(b)) - 1))');
      break;
    case 'one_way_anova': {
      add(...grouped(p));
      add('fit <- aov(y ~ g)', 'print(summary(fit))');
      add('a <- anova(lm(y ~ g)); ss <- a[["Sum Sq"]]; msw <- ss[2] / a$Df[2]');
      add('print(c(total_ss = sum(ss), total_df = sum(a$Df), eta_squared = ss[1] / sum(ss), omega_squared = (ss[1] - a$Df[1] * msw) / (sum(ss) + msw)))');
      add('print(oneway.test(y ~ g))', 'print(levene(y, g))');
      if (p.groups.length >= 3) {
        const method = o.posthoc ?? ADJUST[o.adjustment ?? 'holm'] ?? 'holm';
        add(`print(posthoc_pairs(y, g, ${q(method)}, ${lvl}))`);
      }
      break;
    }
    case 'kruskal_wallis':
      add(...grouped(p));
      add('k <- kruskal.test(y ~ g); print(k)', 'print(tapply(rank(y), g, mean))', 'print(c(epsilon_squared = unname(k$statistic) / (length(y) - 1)))');
      if (p.groups.length >= 3) add(`print(dunn_pairs(y, g, ${q(ADJUST[o.adjustment ?? 'holm'] ?? 'holm')}))`);
      break;
    case 'chi_square':
    case 'fisher': {
      add(`xr <- ${asVal(v0)}; xc <- ${asVal(v1)}`, 'ok <- !is.na(xr) & !is.na(xc)');
      add(`tab <- table(factor(xr[ok], levels = ${vec(p.rowValues ?? v0.categories)}), factor(xc[ok], levels = ${vec(p.colValues ?? v1.categories)}))`, 'print(addmargins(tab))');
      if (p.procedure === 'fisher') {
        add(`print(fisher.test(tab, conf.level = ${lvl}))`, `print(fisher_precise(tab, ${lvl}))`);
      } else {
        add('x <- suppressWarnings(chisq.test(tab, correct = FALSE)); print(x)', 'print(x$expected)', 'print(x$stdres)', 'print(chi_extras(tab, x))');
        add('if (all(dim(tab) == 2)) print(chisq.test(tab, correct = TRUE))');
      }
      break;
    }
    case 'pearson':
    case 'spearman':
    case 'kendall':
      add(...take([['x', asNum(v0)], ['y', asNum(v1)]]));
      add('print(c(n = length(x)))', `print(suppressWarnings(cor.test(x, y, method = ${q(p.procedure)}, conf.level = ${lvl})))`);
      break;
    case 'linear_regression': {
      const { lines, count, stepList } = modelSetup(p, false);
      add(...lines);
      stepList.forEach((_, i) => {
        const fit = `fit${i + 1}`;
        add(`print(summary(${fit}))`, `print(confint(${fit}, level = ${lvl}))`, `print(std_beta(${fit}))`, `vif <- vif_values(${fit}); if (!is.null(vif)) print(vif)`);
        if (i > 0) add(`print(c(r_squared_change = summary(${fit})$r.squared - summary(fit${i})$r.squared))`);
      });
      if (count > 1) add(`print(anova(${stepList.map((_, i) => `fit${i + 1}`).join(', ')}))`);
      add(`print(head(sort(cooks.distance(fit${count}), decreasing = TRUE), 5))`);
      break;
    }
    case 'logistic_regression': {
      const { lines, count, stepList } = modelSetup(p, true);
      add(...lines);
      stepList.forEach((_, i) => {
        const fit = `fit${i + 1}`;
        const last = i === stepList.length - 1;
        add(`print(summary(${fit})$coefficients)`, `print(exp(cbind(odds_ratio = coef(${fit}), confint.default(${fit}, level = ${lvl}))))`);
        if (last && p.profile) add(`print(profile_ci(${fit}, ${lvl}))`);
        add(
          `print(c(deviance = deviance(${fit}), null_deviance = ${fit}$null.deviance, AIC = AIC(${fit}), BIC = BIC(${fit}), n = nobs(${fit})))`,
          `print(c(chi2 = ${fit}$null.deviance - deviance(${fit}), df = ${fit}$df.null - ${fit}$df.residual, p = pchisq(${fit}$null.deviance - deviance(${fit}), ${fit}$df.null - ${fit}$df.residual, lower.tail = FALSE)))`,
        );
        if (last) add(`print(pseudo_r2(${fit}))`, `hosmer_lemeshow(${fit})`, `classify_auc(${fit})`);
      });
      if (count > 1) add(`print(anova(${stepList.map((_, i) => `fit${i + 1}`).join(', ')}, test = "Chisq"))`);
      const fitLast = `fit${count}`;
      add(`print(drop1(${fitLast}, test = "Chisq"))`);
      add(`ck <- cooks.distance(${fitLast}); top <- head(order(ck, decreasing = TRUE), 10)`, `print(data.frame(row = top, fitted = fitted(${fitLast})[top], leverage = hatvalues(${fitLast})[top], cooks = ck[top], standardized = rstandard(${fitLast})[top]))`);
      break;
    }
    case 'curve_fit': {
      add(...take([['y', asNum(v0)], ['x', asNum(v1)]]));
      add('print(c(n = length(x)))', 'd <- data.frame(x = x, y = y)');
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
      const drawn = p.charts?.some((c) => c.chart.kind === 'curves');
      if (drawn) add('fits <- list()');
      for (const kind of p.curveKinds ?? Object.keys(formulas)) {
        if (!formulas[kind]) continue;
        const logged = kind === 'power' || kind === 'exponential' || kind === 's_curve';
        add(`cat("\\n", ${q(kind)}, "\\n", sep = "")`, `fit <- lm(${formulas[kind]}, data = d)`, 'print(summary(fit))', `print(confint(fit, level = ${lvl}))`, `print(curve_original(fit, y, ${logged ? 'TRUE' : 'FALSE'}))`);
        if (kind === 'power' || kind === 'exponential') add('print(c(a = unname(exp(coef(fit)[1])), b = unname(coef(fit)[2])))');
        if (drawn) add(`fits[[${q(kind)}]] <- fit`);
      }
      break;
    }
    case 'shapiro_wilk':
      add(...take([['x', asNum(v0)]]), 'print(c(n = length(x)))', 'print(shapiro.test(x))');
      break;
    case 'ks_normal':
      add(...take([['x', asNum(v0)]]));
      add('print(c(n = length(x)))');
      if (p.estimated) add('print(c(mean = mean(x), sd = sd(x)))', 'print(suppressWarnings(ks.test(x, "pnorm", mean(x), sd(x))))');
      else add(`print(c(mean = ${rn(p.mean ?? o.mu ?? 0)}, sd = ${rn(p.sd ?? o.sd ?? 1)}))`, `print(suppressWarnings(ks.test(x, "pnorm", ${rn(p.mean ?? o.mu ?? 0)}, ${rn(p.sd ?? o.sd ?? 1)})))`);
      break;
    case 'runs':
      if (v0.numeric) {
        const cut = o.cut ?? 'median';
        add(...take([['x', asNum(v0)]]));
        add(`cut <- ${cut === 'median' ? 'median(x)' : cut === 'mean' ? 'mean(x)' : rn(o.mu ?? 0)}`, 'print(c(cut = cut))', 'print(runs_test(x >= cut))');
      } else {
        add(...take([['x', asVal(v0)]]));
        add(`print(runs_test(x == ${q(v0.categories[1] ?? '')}))`);
      }
      break;
    case 'binomial': {
      const success = o.success ?? p.counts?.[0] ?? v0.categories[0] ?? '';
      add(...take([['x', asVal(v0)]]));
      add(`print(binom.test(sum(x == ${q(success)}), length(x), p = ${rn(o.proportion ?? 0.5)}, conf.level = ${lvl}))`);
      break;
    }
    case 'goodness_of_fit': {
      const cats = p.counts ?? v0.categories;
      add(...take([['x', asVal(v0)]]), `tab <- table(factor(x, levels = ${vec(cats)}))`);
      const shares = o.expected ? `c(${cats.map((c) => rn(o.expected?.[c] ?? 0)).join(', ')})` : `rep(1, ${cats.length})`;
      add(`shares <- ${shares}; shares <- shares / sum(shares)`);
      add('gf <- suppressWarnings(chisq.test(tab, p = shares)); print(c(n = sum(tab))); print(tab); print(round(100 * shares, 1)); print(gf); print(gf$expected); print(gf$residuals)');
      break;
    }
    case 'sign':
      if (p.vars.length === 2) add(...take([['x', asNum(v0)], ['y', asNum(v1)]]), 'dif <- x - y');
      else add(...take([['x', asNum(v0)]]), `dif <- x - ${rn(o.mu ?? 0)}`);
      add('pos <- sum(dif > 0); neg <- sum(dif < 0)', 'print(c(above = pos, below = neg, equal = sum(dif == 0), median = median(dif)))', `print(binom.test(pos, pos + neg, 0.5, conf.level = ${lvl}))`);
      break;
    case 'friedman':
      add(`m <- cbind(${p.vars.map(asNum).join(', ')})`, `colnames(m) <- ${vec(p.vars.map((v) => v.label))}`, 'm <- m[complete.cases(m), , drop = FALSE]');
      add('f <- friedman.test(m); print(f)', 'print(c(kendall_w = unname(f$statistic) / (nrow(m) * (ncol(m) - 1))))', 'print(colMeans(t(apply(m, 1, rank))))', 'print(apply(m, 2, median))');
      break;
    case 'ks_two':
      add(...grouped(p), 'a <- y[g == levels(g)[1]]; b <- y[g == levels(g)[2]]', 'print(suppressWarnings(ks.test(a, b)))');
      break;
    case 'median_test':
      add(...grouped(p), 'r <- median_test(y, g)', 'print(r$median); print(r$table); print(r$chisq)', 'if (!is.null(r$fisher_p)) print(c(fisher_p = r$fisher_p))');
      break;
    case 'moses': {
      const control = o.control ?? p.groups[0];
      add(...grouped(p), `control <- y[g == ${q(control)}]; test <- y[g != ${q(control)}]`, `print(moses_test(control, test${o.trim !== undefined ? `, ${o.trim}` : ''}))`);
      break;
    }
    case 'mcnemar': {
      const values = p.values ?? v0.categories;
      add(`xa <- ${asVal(v0)}; xb <- ${asVal(v1)}`, 'ok <- !is.na(xa) & !is.na(xb)');
      add(`tab <- table(factor(xa[ok], levels = ${vec(values)}), factor(xb[ok], levels = ${vec(values)}))`, 'print(tab)');
      add('print(mcnemar.test(tab))', 'print(mcnemar.test(tab, correct = FALSE))', 'print(c(exact_p = binom.test(tab[1, 2], tab[1, 2] + tab[2, 1], 0.5)$p.value))');
      break;
    }
    case 'ordinal_association':
      add(`xr <- ${asVal(v0)}; xc <- ${asVal(v1)}`, 'ok <- !is.na(xr) & !is.na(xc)');
      add(`tab <- table(factor(xr[ok], levels = ${vec(p.rowValues ?? v0.categories)}), factor(xc[ok], levels = ${vec(p.colValues ?? v1.categories)}))`, 'print(tab)');
      add('r <- ordinal_assoc(tab); print(r); print(attr(r, "pairs"))');
      break;
    case 'partial_correlation': {
      const controls = p.vars.slice(2);
      add(...take([['x', asNum(v0)], ['y', asNum(v1)], ...controls.map((c, i): [string, string] => [`z${i + 1}`, asNum(c)])]));
      add('print(c(n = length(x)))', `print(partial_cor(x, y, cbind(${controls.map((_, i) => `z${i + 1}`).join(', ')}), ${lvl}))`);
      break;
    }
    case 'two_way_anova': {
      const [, va, vb] = p.vars;
      add(...take([['y', asNum(v0)], ['a', asVal(va)], ['b', asVal(vb)]]));
      add(`A <- factor(a, levels = ${vec(p.levelsA ?? va.categories)}); B <- factor(b, levels = ${vec(p.levelsB ?? vb.categories)})`, 'print(c(n = length(y)))');
      add('fit <- lm(y ~ A * B, contrasts = list(A = "contr.sum", B = "contr.sum"))', 'print(drop1(fit, scope = ~ A + B + A:B, test = "F"), digits = 10)');
      add('dd <- drop1(fit, scope = ~ A + B + A:B, test = "F"); rss <- dd$RSS[1]; dfe <- fit$df.residual', 'sq <- dd[["Sum of Sq"]][2:4]; df <- dd$Df[2:4]');
      add('print(rbind(ss = sq, df = df, ms = sq / df, f = (sq / df) / (rss / dfe), p = dd[["Pr(>F)"]][2:4], partial_eta_squared = sq / (sq + rss)))');
      add('print(c(error_ss = rss, error_df = dfe, error_ms = rss / dfe, total_ss = sum((y - mean(y))^2), total_df = length(y) - 1))', 'print(c(r_squared = summary(fit)$r.squared, adj_r_squared = summary(fit)$adj.r.squared))');
      add('cell <- factor(paste(A, B), levels = paste(rep(levels(A), each = nlevels(B)), rep(levels(B), times = nlevels(A))))', 'g <- cell', 'print(sapply(split(y, cell), function(v) c(n = length(v), mean = mean(v), sd = sd(v))))');
      add('print(sapply(split(y, A), function(v) c(n = length(v), mean = mean(v))))', 'print(sapply(split(y, B), function(v) c(n = length(v), mean = mean(v))))');
      add('dev <- abs(y - ave(y, cell, FUN = median)); print(anova(lm(dev ~ cell)))');
      break;
    }
    case 'cronbach_alpha': {
      add(`m <- cbind(${p.vars.map(asNum).join(', ')})`, `colnames(m) <- ${vec(p.vars.map((v) => v.label))}`, 'm <- m[complete.cases(m), , drop = FALSE]');
      add('k <- ncol(m); n <- nrow(m); S <- cov(m); R <- cor(m); rbar <- mean(R[upper.tri(R)])', 'alpha <- k / (k - 1) * (1 - sum(diag(S)) / sum(S))');
      add(`tail <- (1 - ${lvl}) / 2; lower <- 1 - (1 - alpha) * qf(1 - tail, n - 1, (n - 1) * (k - 1)); upper <- 1 - (1 - alpha) * qf(tail, n - 1, (n - 1) * (k - 1))`);
      add('print(c(n = n, k = k, alpha = alpha, standardized_alpha = k * rbar / (1 + (k - 1) * rbar), mean_inter_item_r = rbar, lower = lower, upper = upper))', 'print(c(total_mean = mean(rowSums(m)), total_sd = sd(rowSums(m))))');
      add('print(rbind(mean = colMeans(m), sd = apply(m, 2, sd), item_rest_r = sapply(seq_len(k), function(j) cor(m[, j], rowSums(m[, -j, drop = FALSE]))), alpha_if_deleted = sapply(seq_len(k), function(j) if (k == 2) NA else { s2 <- cov(m[, -j, drop = FALSE]); (k - 1) / (k - 2) * (1 - sum(diag(s2)) / sum(s2)) })))');
      break;
    }
    default:
      add(`stop(${q(`no R code is written for ${p.procedure}`)})`);
  }
  add(...bootCode(p), ...wholeCode(p), ...chartCode(p));
  return L.join('\n');
}
