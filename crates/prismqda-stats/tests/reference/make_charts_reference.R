# Reference values for the numbers behind the charts, computed by base R (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_charts_reference.R
# It writes charts_reference.json next to this script. tests/charts_reference.rs requires PrismQDA's chart numbers to match.
#
#   histogram   the bin edges follow PrismQDA's rule (Sturges' number of bins, the width rounded up to 1, 2, 5, or 10 times a power
#               of ten, edges rounded to 12 digits), written out here in R as a second, independent implementation; the counts are
#               then R's own: hist(x, breaks = edges, right = TRUE, include.lowest = TRUE)$counts
#   box         quartiles by quantile(type = 7), whiskers and outliers by the 1.5 interquartile range rule of boxplot.stats, applied
#               to those quartiles
#   normal Q-Q  the points of qqnorm (x = qnorm(ppoints(n)), y = the sorted values) and the line of qqline (through the quartiles)
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

edges_for <- function(x) {
  mn <- min(x); mx <- max(x)
  if (mn == mx) return(c(signif(mn - 0.5, 12), signif(mn + 0.5, 12)))
  bins <- max(3, ceiling(log2(length(x))) + 1)
  raw <- (mx - mn) / bins
  mag <- 10^floor(log10(raw)); r <- raw / mag
  width <- (if (r <= 1) 1 else if (r <= 2) 2 else if (r <= 5) 5 else 10) * mag
  start <- signif(floor(mn / width) * width, 12)
  edges <- start; i <- 1
  while (tail(edges, 1) < mx) { edges <- c(edges, signif(start + i * width, 12)); i <- i + 1 }
  edges
}

one <- function(name, x) {
  edges <- edges_for(x)
  h <- hist(x, breaks = edges, right = TRUE, include.lowest = TRUE, plot = FALSE)
  q <- quantile(x, c(.25, .5, .75), type = 7, names = FALSE)
  iqr <- q[3] - q[1]
  inside <- x[x >= q[1] - 1.5 * iqr & x <= q[3] + 1.5 * iqr]
  out <- sort(x[x < q[1] - 1.5 * iqr | x > q[3] + 1.5 * iqr])
  qq <- qqnorm(x, plot.it = FALSE)
  o <- order(qq$x)
  yq <- quantile(x, c(.25, .75), names = FALSE, type = 7); xq <- qnorm(c(.25, .75))
  slope <- diff(yq) / diff(xq); int <- yq[1L] - slope * xq[1L]
  list(name = name, x = x,
       histogram = list(edges = num(edges), counts = num(h$counts)),
       box = list(q1 = q[1], median = q[2], q3 = q[3], lower_whisker = min(inside), upper_whisker = max(inside), mean = mean(x), outliers = num(out)),
       qq = list(theoretical = num(qq$x[o]), sample = num(qq$y[o]), intercept = int, slope = slope))
}

set.seed(77)
result <- list(
  one("twelve_ages", c(21, 25, 33, 41, 29, 38, 44, 52, 31, 36, 47, 55)),
  one("skewed_forty", round(rexp(40, 0.1), 2)),
  one("seven_values", round(rnorm(7, 10, 3), 1)),
  one("with_outliers", c(round(rnorm(30, 50, 5), 1), 85, 12)),
  one("decimals_on_edges", c(0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.25, 0.75)),
  one("large_numbers", round(rnorm(100, 52000, 9000))),
  one("all_the_same", rep(4, 6))
)
path <- file.path(script_dir(), "charts_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
