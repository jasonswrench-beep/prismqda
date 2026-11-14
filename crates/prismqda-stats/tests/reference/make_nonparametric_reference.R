# Reference values for the nonparametric tests of the 1.0 plan (binomial, sign, McNemar, Friedman, one-sample chi-square, the
# median test, Kolmogorov-Smirnov with one and two samples, the runs test, and the Moses test) and for partial correlation,
# computed by base R only (no packages except jsonlite, to write the file).
#
# Run with:  Rscript make_nonparametric_reference.R
# It writes nonparametric_reference.json next to this script. tests/nonparametric_reference.rs requires PrismQDA to match.
#
# Where R has a function it is the reference (`binom.test`, `mcnemar.test`, `friedman.test`, `chisq.test`, `ks.test`). Where it has
# none in base R (the runs test, the median test, the Moses test, partial correlation) the quantity is written out here from its
# definition, and the Moses test's exact distribution is found by listing every arrangement of the ranks.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
set.seed(20261108)
r3 <- function(x) round(x, 3)

# ---------------------------------------------------------------- binomial and sign
binom_cases <- list(c(7, 10, .5), c(0, 5, .3), c(5, 5, .5), c(13, 40, .25), c(60, 100, .5), c(3, 12, .1), c(45, 50, .8), c(1, 2, .5), c(18, 30, .5), c(250, 400, .6))
binom <- lapply(binom_cases, function(a) {
  lapply(c(0.9, 0.95, 0.99), function(level) { b <- binom.test(a[1], a[2], a[3], conf.level = level); list(level = level, x = a[1], n = a[2], p0 = a[3], p = b$p.value, estimate = unname(b$estimate), lo = b$conf.int[1], hi = b$conf.int[2]) })
})
binom <- do.call(c, binom)

sign_samples <- list(
  c(1.2, -0.4, 2.2, 0.9, 3.1, 0, 1.7, -1.1, 2.6, 0.3), c(-2, -1, -3, -4, 0, -2.5, -1.5), c(0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, -0.2),
  c(round(rnorm(30, 0.2, 1), 2)), c(0, 0, 0, 1, -1), c(round(rnorm(80, 0.1, 1), 2))
)
sign <- lapply(sign_samples, function(x) {
  nz <- x[x != 0]; pos <- sum(nz > 0); neg <- sum(nz < 0)
  b <- binom.test(pos, pos + neg, 0.5)
  list(x = x, positive = pos, negative = neg, zeros = sum(x == 0), p = b$p.value, median = median(x))
})

# ---------------------------------------------------------------- McNemar
mc_tables <- list(c(30, 12, 5, 28), c(10, 9, 3, 20), c(50, 3, 14, 40), c(8, 5, 5, 7), c(100, 22, 22, 56), c(12, 0, 6, 9), c(20, 7, 7, 20))
mcnemar <- lapply(mc_tables, function(v) {
  m <- matrix(v, 2, 2, byrow = TRUE)
  cc <- mcnemar.test(m, correct = TRUE); nc <- mcnemar.test(m, correct = FALSE)
  b <- m[1, 2]; cc_n <- m[2, 1]
  ex <- if (b + cc_n > 0) binom.test(b, b + cc_n, 0.5)$p.value else NA
  list(table = v, b = b, c = cc_n, statistic_corrected = unname(cc$statistic), p_corrected = cc$p.value, statistic = unname(nc$statistic), p = nc$p.value, p_exact = ex)
})

# ---------------------------------------------------------------- Friedman
friedman_sets <- list(
  matrix(c(9, 6, 9, 7, 8, 6, 5, 6, 3, 2, 7, 7, 4, 3, 5, 6, 8, 5, 7, 7), 10, 2, byrow = FALSE),
  matrix(round(rnorm(36, 5, 2), 1), 12, 3), matrix(round(rnorm(40, 5, 2), 1), 10, 4), matrix(sample(1:5, 60, TRUE), 12, 5),
  matrix(c(1, 2, 3, 2, 3, 1, 3, 1, 2, 1, 3, 2, 2, 1, 3, 3, 2, 1), 6, 3, byrow = TRUE), matrix(round(rnorm(90, 10, 3)), 15, 6)
)
friedman <- lapply(friedman_sets, function(m) {
  f <- friedman.test(m); n <- nrow(m); k <- ncol(m)
  ranks <- t(apply(m, 1, rank))
  list(data = m, statistic = unname(f$statistic), df = unname(f$parameter), p = f$p.value, kendall_w = unname(f$statistic) / (n * (k - 1)), mean_ranks = colMeans(ranks))
})

# ---------------------------------------------------------------- one-sample chi-square
gof_cases <- list(list(x = c(18, 22, 20, 40), p = NULL), list(x = c(50, 30, 20), p = c(.5, .3, .2)), list(x = c(8, 12), p = c(.25, .75)), list(x = c(5, 9, 6, 12, 8), p = NULL), list(x = c(100, 0, 40), p = c(.6, .1, .3)))
gof <- lapply(gof_cases, function(g) {
  pp <- if (is.null(g$p)) rep(1 / length(g$x), length(g$x)) else g$p
  r <- suppressWarnings(chisq.test(g$x, p = pp))
  list(x = g$x, p_expected = pp, statistic = unname(r$statistic), df = unname(r$parameter), p = r$p.value, expected = r$expected, residuals = as.vector(r$residuals))
})

# ---------------------------------------------------------------- median test (Mood): above the grand median against at or below it
median_sets <- list(
  list(c(12, 15, 11, 18, 20, 14, 16), c(22, 25, 19, 24, 27, 21)), list(c(3, 5, 4, 6, 2, 5), c(7, 9, 8, 6, 10, 9), c(5, 6, 7, 4, 6, 5, 8)),
  list(round(rnorm(20, 10, 3), 1), round(rnorm(25, 12, 3), 1), round(rnorm(18, 9, 3), 1), round(rnorm(22, 11, 3), 1)), list(c(1, 2, 3, 4, 5), c(3, 4, 5, 6, 7))
)
median_test <- lapply(median_sets, function(g) {
  all <- unlist(g); md <- median(all)
  above <- sapply(g, function(x) sum(x > md)); below <- sapply(g, function(x) sum(x <= md))
  tab <- rbind(above, below)
  ct <- suppressWarnings(chisq.test(tab, correct = FALSE))
  list(groups = g, median = md, above = above, below = below, statistic = unname(ct$statistic), df = unname(ct$parameter), p = ct$p.value,
       fisher_p = if (length(g) == 2) fisher.test(tab)$p.value else NA)
})

# ---------------------------------------------------------------- Kolmogorov-Smirnov
one_sample <- lapply(list(c(0.1, -0.8, 1.2, 0.4, -0.3, 2.1, -1.4, 0.9), round(rnorm(30), 3), round(rnorm(60, 0.3, 1.2), 3), round(rnorm(99), 3), round(rnorm(150, 0, 1), 3), round(runif(40, -2, 2), 3)), function(x) {
  e <- suppressWarnings(ks.test(x, "pnorm", 0, 1)); a <- suppressWarnings(ks.test(x, "pnorm", 0, 1, exact = FALSE))
  list(x = x, d = unname(e$statistic), p = e$p.value, p_asymptotic = a$p.value, exact = length(x) < 100)
})
two_sets <- list(
  list(c(1.1, 2.3, 3.2, 4.8, 5.1, 6.7), c(2.2, 3.9, 4.4, 7.2, 8.1, 9.3, 10.5)), list(round(rnorm(20), 3), round(rnorm(25, 0.5), 3)), list(round(rnorm(40), 3), round(rnorm(35, 0, 2), 3)),
  list(c(1, 2, 2, 3, 3, 3, 4, 5), c(2, 3, 4, 4, 5, 5, 6, 6, 7)), list(sample(1:6, 30, TRUE), sample(2:7, 28, TRUE)), list(round(rnorm(120), 3), round(rnorm(100, 0.3), 3))
)
two_sample <- lapply(two_sets, function(s) {
  e <- suppressWarnings(ks.test(s[[1]], s[[2]])); a <- suppressWarnings(ks.test(s[[1]], s[[2]], exact = FALSE))
  list(x = s[[1]], y = s[[2]], d = unname(e$statistic), p = e$p.value, p_asymptotic = a$p.value, exact = length(s[[1]]) * length(s[[2]]) < 10000)
})

# ---------------------------------------------------------------- runs test: a sequence made two-valued, written out
runs_cases <- list(c(1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1), c(1, 0, 1, 0, 1, 0, 1, 0, 1, 0), c(1, 1, 1, 1, 1, 0, 0, 0, 0, 0), c(rbinom(40, 1, .5)), c(rbinom(60, 1, .3)), c(1, 1, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1))
runs <- lapply(runs_cases, function(s) {
  n1 <- sum(s == 1); n0 <- sum(s == 0); n <- n1 + n0
  nr <- 1 + sum(s[-1] != s[-length(s)])
  mu <- 2 * n1 * n0 / n + 1
  sigma <- sqrt(2 * n1 * n0 * (2 * n1 * n0 - n) / (n^2 * (n - 1)))
  cc <- if (nr > mu) -0.5 else if (nr < mu) 0.5 else 0
  z <- (nr - mu + cc) / sigma
  # the exact distribution of the number of runs
  dens <- function(r) {
    if (r %% 2 == 0) { k <- r / 2; 2 * choose(n1 - 1, k - 1) * choose(n0 - 1, k - 1) / choose(n, n1) }
    else { k <- (r - 1) / 2; (choose(n1 - 1, k) * choose(n0 - 1, k - 1) + choose(n1 - 1, k - 1) * choose(n0 - 1, k)) / choose(n, n1) }
  }
  rs <- 2:min(n, 2 * min(n1, n0) + 1)
  d <- sapply(rs, dens)
  lower <- sum(d[rs <= nr]); upper <- sum(d[rs >= nr])
  list(sequence = s, runs = nr, n1 = n1, n0 = n0, mean = mu, sd = sigma, z = z, p_normal = 2 * pnorm(-abs(z)), p_exact = min(1, 2 * min(lower, upper)))
})

# ---------------------------------------------------------------- Moses extreme reaction: every arrangement is listed
moses_sets <- list(
  list(control = c(4.1, 5.2, 3.8, 6.0, 4.7, 5.5, 4.4), test = c(2.9, 7.4, 3.1, 8.2, 6.9, 1.8, 7.8)),
  list(control = c(10, 12, 11, 13, 12.5, 9.8, 11.5, 12.2, 10.9), test = c(14, 8, 9, 15, 7, 12.1, 16)),
  list(control = c(2.2, 3.4, 2.9, 3.1, 2.7, 3.0, 3.3, 2.6, 2.8, 3.2, 2.5), test = c(1.9, 4.5, 3.6))
)
moses <- lapply(moses_sets, function(s) {
  nc <- length(s$control); nt <- length(s$test); N <- nc + nt
  pooled <- c(s$control, s$test); rk <- rank(pooled); ctrl_rank <- sort(rk[1:nc])
  span_of <- function(positions, h) { kept <- sort(positions)[(h + 1):(length(positions) - h)]; max(kept) - min(kept) + 1 }
  all_combos <- combn(N, nc)
  dist_for <- function(h) apply(all_combos, 2, span_of, h = h)
  h5 <- max(1, floor(0.05 * nc))
  out <- list(control = s$control, test = s$test, h = h5)
  for (h in c(0, h5)) {
    obs <- span_of(ctrl_rank, h)
    d <- dist_for(h)
    out[[paste0("span_h", h)]] <- obs
    out[[paste0("p_h", h)]] <- mean(d <= obs)
  }
  out
})

# ---------------------------------------------------------------- partial correlation
pc_sets <- list(
  { z1 <- round(rnorm(30), 2); x <- round(0.7 * z1 + rnorm(30, 0, .6), 2); y <- round(0.5 * z1 + 0.3 * x + rnorm(30, 0, .7), 2); list(x = x, y = y, z = list(z1)) },
  { z1 <- round(rnorm(40), 2); z2 <- round(rnorm(40), 2); x <- round(z1 - 0.5 * z2 + rnorm(40, 0, .8), 2); y <- round(0.6 * z1 + 0.4 * z2 + 0.4 * x + rnorm(40, 0, .8), 2); list(x = x, y = y, z = list(z1, z2)) },
  { z1 <- round(rnorm(12), 2); x <- round(z1 + rnorm(12), 2); y <- round(-z1 + rnorm(12), 2); list(x = x, y = y, z = list(z1)) }
)
partial <- lapply(pc_sets, function(s) {
  n <- length(s$x); k <- length(s$z)
  Z <- do.call(cbind, s$z)
  rx <- resid(lm(s$x ~ Z)); ry <- resid(lm(s$y ~ Z))
  r <- cor(rx, ry)
  # the same from the inverse of the correlation matrix
  M <- cor(cbind(s$x, s$y, Z)); P <- solve(M); r2 <- -P[1, 2] / sqrt(P[1, 1] * P[2, 2])
  stopifnot(abs(r - r2) < 1e-10)
  df <- n - 2 - k; t <- r * sqrt(df / (1 - r^2)); p <- 2 * pt(-abs(t), df)
  zc <- atanh(r); se <- 1 / sqrt(n - 3 - k)
  list(x = s$x, y = s$y, z = s$z, n = n, r = r, df = df, t = t, p = p, lo = tanh(zc - qnorm(0.975) * se), hi = tanh(zc + qnorm(0.975) * se))
})

out <- list(binomial = binom, sign = sign, mcnemar = mcnemar, friedman = friedman, gof = gof, median_test = median_test,
            ks_one = one_sample, ks_two = two_sample, runs = runs, moses = moses, partial = partial)
path <- file.path(script_dir(), "nonparametric_reference.json")
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
