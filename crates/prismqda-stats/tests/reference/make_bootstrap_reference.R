# Reference values for the bootstrap intervals, computed with R's boot package (version 1.3.x, which ships with R).
#
# Run with:  Rscript make_bootstrap_reference.R
# It writes bootstrap_reference.json next to this script. tests/bootstrap_reference.rs gives PrismQDA the same resampled values
# that R drew, and requires the same intervals back.
#
# R's random number generator is not PrismQDA's, so the resamples themselves cannot be compared; what can be compared, and is,
# is everything that happens after them: the standard error and bias, the percentile interval, and the BCa interval, which
# boot.ci works out from the resampled values with the interpolation of boot:::norm.inter. The BCa acceleration is given to
# boot.ci as the jackknife influence values (L), written out here as (n - 1) * (estimate - the estimate without case i).
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages({ library(boot); library(jsonlite) })

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

run <- function(name, data, statistic, R, seed, conf) {
  set.seed(seed)
  b <- boot(data, statistic, R = R)
  n <- NROW(data)
  t0 <- b$t0[1]
  jack <- sapply(seq_len(n), function(i) statistic(data[-i, , drop = FALSE], seq_len(n - 1))[1])
  L <- (n - 1) * (t0 - jack)
  ci <- boot.ci(b, conf = conf, type = c("perc", "bca"), L = L)
  list(name = name, level = conf, resamples = R, estimate = t0, replicates = num(b$t[, 1]), influence = num(L),
       se = sd(b$t[, 1]), bias = mean(b$t[, 1]) - t0,
       percentile = num(ci$percent[4:5]), bca = num(ci$bca[4:5]))
}

set.seed(31)
skewed <- data.frame(x = round(rexp(25, 0.2), 2))
paired <- data.frame(u = round(rnorm(30, 50, 10), 1))
paired$v <- round(0.6 * paired$u + rnorm(30, 0, 8), 1)
small <- data.frame(x = round(rnorm(12, 5, 2), 2))

stat_mean <- function(d, i) mean(d[i, 1])
stat_median <- function(d, i) median(d[i, 1])
stat_sd <- function(d, i) sd(d[i, 1])
stat_cor <- function(d, i) cor(d[i, 1], d[i, 2])
stat_diff <- function(d, i) mean(d[i, 1] - d[i, 2])

result <- list(
  list(run("mean_of_a_skewed_variable", skewed, stat_mean, 999, 1, 0.95), data = num(skewed$x)),
  list(run("median_of_a_skewed_variable", skewed, stat_median, 999, 2, 0.95), data = num(skewed$x)),
  list(run("standard_deviation", skewed, stat_sd, 1999, 3, 0.90), data = num(skewed$x)),
  list(run("correlation_of_pairs", paired, stat_cor, 1999, 4, 0.95), data = list(num(paired$u), num(paired$v))),
  list(run("mean_difference_of_pairs", paired, stat_diff, 999, 5, 0.99), data = list(num(paired$u), num(paired$v))),
  list(run("small_sample_few_resamples", small, stat_mean, 199, 6, 0.95), data = num(small$x))
)
result <- lapply(result, function(r) { r[[1]]$data <- r$data; r[[1]] })

path <- file.path(script_dir(), "bootstrap_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "| boot", as.character(packageVersion("boot")), "\n")
