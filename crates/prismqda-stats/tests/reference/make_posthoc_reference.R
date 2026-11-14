# Reference values for the post-hoc tests, the Shapiro-Wilk test, the exact Spearman and Kendall tests, and the exact intervals of
# Cohen's d, computed by base R only (no packages except jsonlite, to write the file).
#
# Run with:  Rscript make_posthoc_reference.R
# It writes posthoc_reference.json next to this script. tests/posthoc_reference.rs requires PrismQDA's functions to match.
#
# Games-Howell and Scheffe are not in base R, so they are written out here from their published formulas with R's ptukey,
# qtukey, pt, and pf. Tukey's test is R's own TukeyHSD. All the data are invented.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}

# ---------------------------------------------------------------- the studentized range distribution
qgrid <- c(0.05, 0.4, 1, 1.8, 2.9, 3.5, 4.4, 6, 9, 15)
kgrid <- c(2, 3, 4, 5, 8, 12)
dfgrid <- c(2, 3, 5, 10, 20, 60, 120, 500, 3000, 40000)
ptuk <- do.call(c, lapply(kgrid, function(k) do.call(c, lapply(dfgrid, function(d) lapply(qgrid, function(q) c(q, k, d, ptukey(q, k, d)))))))
pgrid <- c(0.5, 0.8, 0.9, 0.95, 0.99)
# R's own qtukey stops when two steps differ by less than 1e-4, so the "exact" column solves ptukey = p to full accuracy.
exact_q <- function(p, k, d) uniroot(function(q) ptukey(q, k, d) - p, c(0.001, 60), tol = 1e-13)$root
qtuk <- do.call(c, lapply(c(2, 3, 5, 10), function(k) do.call(c, lapply(c(3, 8, 20, 100, 1000), function(d) lapply(pgrid, function(p) c(p, k, d, qtukey(p, k, d), exact_q(p, k, d)))))))

# ---------------------------------------------------------------- one-way comparisons of pairs
groups_sets <- list(
  list(name = "balanced three, similar spread", y = list(c(10, 12, 11, 13, 12.5, 9.8), c(15, 17, 16, 18, 15.5, 16.2), c(20, 22, 21, 23, 19.5, 21.4))),
  list(name = "unbalanced four", y = list(c(5.1, 6.2, 4.8, 5.9), c(7.7, 8.1, 6.9, 7.2, 8.4, 7.9, 7.1), c(6.4, 5.8, 6.9), c(9.2, 10.4, 9.9, 11.1, 10.2))),
  list(name = "unequal spread three", y = list(c(10, 10.5, 9.8, 10.2, 10.1), c(12, 18, 7, 15, 9, 20, 4), c(11, 13, 12, 14, 9, 10, 12.5, 11.5))),
  list(name = "two groups", y = list(c(3, 4, 5, 6, 7, 5.5), c(6, 7, 8, 9.5, 8.5, 7.2, 9))),
  list(name = "five small groups", y = list(c(1.2, 2.1, 1.7), c(2.9, 3.4, 2.5, 3.1), c(4.1, 3.7, 4.6), c(2.2, 2.0, 2.6, 2.4), c(5.0, 4.4, 5.6, 4.9)))
)

posthoc <- lapply(groups_sets, function(s) {
  y <- unlist(s$y); g <- factor(rep(seq_along(s$y), sapply(s$y, length)))
  k <- nlevels(g); n <- tapply(y, g, length); m <- tapply(y, g, mean); v <- tapply(y, g, var); N <- length(y)
  fit <- aov(y ~ g); mse <- sum(resid(fit)^2) / (N - k)
  tk <- TukeyHSD(fit, conf.level = 0.95)$g
  rows <- list()
  idx <- 0
  for (i in 1:(k - 1)) for (j in (i + 1):k) {
    idx <- idx + 1
    diff <- m[j] - m[i]
    # Games-Howell
    se_gh <- sqrt(v[i] / n[i] + v[j] / n[j])
    df_gh <- (v[i] / n[i] + v[j] / n[j])^2 / ((v[i] / n[i])^2 / (n[i] - 1) + (v[j] / n[j])^2 / (n[j] - 1))
    t_gh <- diff / se_gh
    p_gh <- ptukey(abs(t_gh) * sqrt(2), k, df_gh, lower.tail = FALSE)
    q_gh <- exact_q(0.95, k, df_gh)
    # Scheffe
    se_sc <- sqrt(mse * (1 / n[i] + 1 / n[j]))
    f_sc <- diff^2 / ((k - 1) * se_sc^2)
    p_sc <- pf(f_sc, k - 1, N - k, lower.tail = FALSE)
    crit_sc <- sqrt((k - 1) * qf(0.95, k - 1, N - k))
    # Tukey (R's own, plus a full-accuracy interval)
    q_tk <- exact_q(0.95, k, N - k)
    se_tk <- sqrt(mse / 2 * (1 / n[i] + 1 / n[j]))
    rows[[idx]] <- list(i = i, j = j, diff = unname(diff),
      tukey = c(unname(tk[idx, "lwr"]), unname(tk[idx, "upr"]), unname(tk[idx, "p adj"]), unname(diff - q_tk * se_tk), unname(diff + q_tk * se_tk)),
      games_howell = c(unname(t_gh), unname(df_gh), unname(p_gh), unname(diff - q_gh / sqrt(2) * se_gh), unname(diff + q_gh / sqrt(2) * se_gh)),
      scheffe = c(unname(f_sc), unname(p_sc), unname(diff - crit_sc * se_sc), unname(diff + crit_sc * se_sc)))
  }
  list(name = s$name, groups = s$y, mse = mse, df_within = N - k, pairs = rows)
})

# ---------------------------------------------------------------- Shapiro-Wilk
set.seed(20261108)
sw_samples <- list(
  c(2.1, 3.4, 1.9), c(1, 2, 4), c(1.1, 2.3, 2.9, 4.4), c(5, 3, 8, 2, 9, 4, 6),
  c(1, 2, 3, 4, 5, 6, 7, 8, 9, 10), round(rnorm(11, 10, 2), 3), round(rnorm(12, 10, 2), 3), round(rnorm(20, 50, 8), 3),
  round(rexp(15), 3), round(rexp(40), 3), round(rnorm(50), 3), round(rnorm(100, 3, 1.5), 3), round(runif(30), 3),
  c(rep(1, 5), rep(2, 5), rep(3, 5), 10), round(rlnorm(25), 3), round(rnorm(500), 3), round(rnorm(2000, 5, 2), 3), round(rt(60, 3), 3)
)
shapiro <- lapply(sw_samples, function(x) { r <- shapiro.test(x); list(x = x, w = unname(r$statistic), p = r$p.value) })

# ---------------------------------------------------------------- exact Spearman and Kendall
ex <- list(
  list(c(1, 2, 3, 4, 5), c(2, 1, 4, 3, 5)), list(c(1, 2, 3, 4, 5, 6), c(6, 5, 4, 3, 2, 1)),
  list(c(3.2, 1.1, 4.7, 2.9, 5.5, 6.1, 0.4), c(2.0, 1.5, 3.9, 2.2, 6.0, 5.1, 0.9)),
  list(c(1, 2, 3, 4, 5, 6, 7, 8), c(2, 1, 3, 5, 4, 8, 6, 7)),
  list(c(5, 3, 8, 2, 9, 4, 6, 1, 7), c(4, 6, 9, 1, 8, 2, 5, 3, 7)),
  list(c(1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5), c(1, 3, 2, 5, 4, 8, 6, 10, 7, 9)),
  list(round(rnorm(12), 3), round(rnorm(12), 3)), list(round(rnorm(15), 3), round(rnorm(15), 3)),
  list(1:20 + round(rnorm(20, 0, 3), 3), 1:20 + round(rnorm(20, 0, 5), 3)),
  list(round(rnorm(30), 3), round(rnorm(30), 3)), list(1:40 + round(rnorm(40, 0, 6), 3), 1:40 + round(rnorm(40, 0, 9), 3)),
  list(round(rnorm(49), 3), round(rnorm(49), 3)), list(1:100 + round(rnorm(100, 0, 20), 3), 1:100 + round(rnorm(100, 0, 30), 3))
)
exact_cor <- lapply(ex, function(p) {
  x <- p[[1]]; y <- p[[2]]
  sp <- suppressWarnings(cor.test(x, y, method = "spearman", exact = TRUE))
  kd <- suppressWarnings(cor.test(x, y, method = "kendall", exact = TRUE))
  list(x = x, y = y, rho = unname(sp$estimate), p_spearman = sp$p.value, tau = unname(kd$estimate), p_kendall = kd$p.value)
})

# ---------------------------------------------------------------- the noncentral t and the exact interval of d
pnt <- do.call(c, lapply(c(2, 5, 12, 40, 200), function(d) do.call(c, lapply(c(-3, -1, 0, 0.8, 2.5, 6), function(ncp) lapply(c(-4, -1, 0.3, 1.7, 3.2, 8), function(q) c(q, d, ncp, pt(q, d, ncp)))))))
ncp_ci <- function(t, df, level) {
  a <- (1 - level) / 2
  lim <- max(40, abs(t) + 40)
  lo <- uniroot(function(ncp) pt(t, df, ncp) - (1 - a), c(-lim, lim), tol = 1e-12)$root
  hi <- uniroot(function(ncp) pt(t, df, ncp) - a, c(-lim, lim), tol = 1e-12)$root
  c(lo, hi)
}
d_cases <- list(
  list(kind = "one", x = c(5.1, 6.3, 4.8, 7.2, 5.9, 6.6, 5.4, 6.1), mu = 5),
  list(kind = "paired", x = c(12, 15, 11, 14, 18, 16, 13, 17, 12.5), y = c(10, 14, 12, 12, 15, 15, 11, 14, 11)),
  list(kind = "independent", x = c(10, 12, 11, 13, 12.5, 9.8), y = c(15, 17, 16, 18, 15.5, 16.2, 14.8)),
  list(kind = "independent", x = c(3, 4, 5, 6, 7, 5.5, 4.2, 6.6, 5.1, 3.9, 6.2, 5.8), y = c(3.4, 4.6, 5.2, 5.9, 7.7, 5.0, 4.4, 6.1, 5.5, 3.8, 6.9, 6.0))
)
dci <- lapply(d_cases, function(cs) {
  if (cs$kind == "one") {
    n <- length(cs$x); tt <- t.test(cs$x, mu = cs$mu); df <- n - 1; scale <- 1 / sqrt(n); d <- (mean(cs$x) - cs$mu) / sd(cs$x)
  } else if (cs$kind == "paired") {
    dd <- cs$x - cs$y; n <- length(dd); tt <- t.test(cs$x, cs$y, paired = TRUE); df <- n - 1; scale <- 1 / sqrt(n); d <- mean(dd) / sd(dd)
  } else {
    n1 <- length(cs$x); n2 <- length(cs$y); tt <- t.test(cs$x, cs$y, var.equal = TRUE); df <- n1 + n2 - 2; scale <- sqrt(1 / n1 + 1 / n2)
    sp <- sqrt(((n1 - 1) * var(cs$x) + (n2 - 1) * var(cs$y)) / df); d <- (mean(cs$x) - mean(cs$y)) / sp
  }
  t <- unname(tt$statistic)
  lists <- lapply(c(0.9, 0.95, 0.99), function(level) { ci <- ncp_ci(t, df, level); c(level, ci[1] * scale, ci[2] * scale) })
  list(kind = cs$kind, x = cs$x, y = if (is.null(cs$y)) NULL else cs$y, mu = if (is.null(cs$mu)) 0 else cs$mu, d = d, t = t, df = df, intervals = lists)
})

out <- list(ptukey = ptuk, qtukey = qtuk, posthoc = posthoc, shapiro = shapiro, exact_cor = exact_cor, pnt = pnt, d_intervals = dci)
path <- file.path(script_dir(), "posthoc_reference.json")
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
