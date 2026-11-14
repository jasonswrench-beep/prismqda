# Reference values for the probability distributions, computed by base R (no packages are needed except jsonlite to write the file).
#
# Run with:  Rscript make_distribution_reference.R
# It writes distribution_reference.json next to this script. tests/distribution_reference.rs requires PrismQDA's functions
# to match these values.
#
# Each entry is a list of rows: the arguments, then R's answer. The grids are chosen to include the places an
# implementation fails: tiny and large degrees of freedom, far tails, and values near the middle.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}

rows <- function(grid, f) unname(lapply(seq_len(nrow(grid)), function(i) {
  r <- as.numeric(grid[i, ])
  c(r, f(r))
}))

x_lgamma <- c(0.01, 0.1, 0.5, 1, 1.5, 2, 3.7, 9.99, 10, 25.5, 100, 1000.25, 1e5)
z <- c(-37, -10, -5.5, -3, -1.96, -1, -0.3, 0, 0.1, 0.8, 1.64, 2.5, 4, 8)
df <- c(1, 2, 3, 5, 10, 30, 100, 1000, 100000)
tt <- c(0, 0.1, 0.7, 1.5, 2.2, 4, 9, 40, -0.5, -2.6, -12)
chi <- c(0.001, 0.2, 1, 3.84, 9, 25, 80, 400)
chidf <- c(1, 2, 3, 6, 20, 100, 500)
fx <- c(0.05, 0.5, 1, 2.5, 4.3, 10, 60)
fd <- rbind(c(1, 5), c(2, 10), c(3, 30), c(5, 5), c(4, 200), c(10, 1000), c(1, 1), c(50, 3))
pq <- c(1e-10, 1e-5, 0.001, 0.025, 0.1, 0.3, 0.5, 0.7, 0.9, 0.975, 0.995, 0.999999)

out <- list(
  lgamma = lapply(x_lgamma, function(x) c(x, lgamma(x))),
  pnorm = lapply(z, function(x) c(x, pnorm(x), pnorm(x, lower.tail = FALSE))),
  qnorm = lapply(pq, function(p) c(p, qnorm(p))),
  pt = rows(expand.grid(t = tt, df = df), function(r) c(pt(r[1], r[2]), 2 * pt(-abs(r[1]), r[2]))),
  qt = rows(expand.grid(p = pq, df = df), function(r) qt(r[1], r[2])),
  pchisq = rows(expand.grid(x = chi, df = chidf), function(r) c(pchisq(r[1], r[2]), pchisq(r[1], r[2], lower.tail = FALSE))),
  qchisq = rows(expand.grid(p = pq[pq < 1 - 1e-6], df = chidf), function(r) qchisq(r[1], r[2])),
  pf = do.call(c, lapply(seq_len(nrow(fd)), function(i) lapply(fx, function(x) c(x, fd[i, 1], fd[i, 2], pf(x, fd[i, 1], fd[i, 2]), pf(x, fd[i, 1], fd[i, 2], lower.tail = FALSE))))),
# R's qf computes 1 / qbeta(p, ., ., lower = FALSE) - 1, which loses all its digits for small p (qf(1e-10, 1, 5) is 0 in R,
  # and the true value is about 1e-20), so only p from 0.025 up is compared; below that PrismQDA is the more accurate one.
  qf = do.call(c, lapply(seq_len(nrow(fd)), function(i) lapply(pq[pq >= 0.025 & pq < 1 - 1e-6], function(p) c(p, fd[i, 1], fd[i, 2], qf(p, fd[i, 1], fd[i, 2])))))
)

path <- file.path(script_dir(), "distribution_reference.json")
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
