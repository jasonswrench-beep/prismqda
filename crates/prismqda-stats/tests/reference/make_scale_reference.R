# Reference values for Cronbach's alpha, computed by base R (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_scale_reference.R
# It writes scale_reference.json next to this script. tests/scale_reference.rs requires PrismQDA's alpha to match these values.
#
# What comes from an R function and what is written out in plain R:
#   alpha                    two ways that must agree to rounding: written out from the covariance matrix, k / (k - 1) * (1 - sum of the
#                            variances / sum of the whole matrix), and Hoyt's form, 1 - MS(residual) / MS(cases) of
#                            anova(lm(score ~ case + item)) on the items stacked one under the other
#   standardized alpha       written out from cor(): k * r / (1 + (k - 1) * r), r the mean of the correlations between items
#   an item's correlation with the rest   cor(item, rowSums(the other items))
#   alpha if an item is left out          the same formula on the other items
#   Feldt's interval         written out: 1 - (1 - alpha) * qf(1 - a/2, n - 1, (n - 1) * (k - 1)) and with qf(a/2, ...)
#
# The data are synthetic and invented here (answers on a 1 to 5 scale from a made-up trait). Nothing comes from a real study.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

alpha_of <- function(X) {
  k <- ncol(X); S <- cov(X)
  k / (k - 1) * (1 - sum(diag(S)) / sum(S))
}

case <- function(name, X, level = 0.95) {
  n <- nrow(X); k <- ncol(X)
  alpha <- alpha_of(X)
  long <- data.frame(score = as.vector(X), item = factor(rep(seq_len(k), each = n)), case = factor(rep(seq_len(n), times = k)))
  a <- anova(lm(score ~ case + item, long))
  hoyt <- 1 - a["Residuals", "Mean Sq"] / a["case", "Mean Sq"]
  R <- cor(X); rbar <- mean(R[upper.tri(R)])
  total <- rowSums(X)
  tail <- (1 - level) / 2
  df1 <- n - 1; df2 <- (n - 1) * (k - 1)
  list(
    name = name, level = level, n = n, k = k, items = lapply(seq_len(k), function(j) num(X[, j])),
    alpha = alpha, alpha_hoyt = hoyt,
    standardized_alpha = k * rbar / (1 + (k - 1) * rbar), mean_inter_item_r = rbar,
    negative_pairs = sum(R[upper.tri(R)] < 0),
    total_mean = mean(total), total_sd = sd(total),
    item_mean = num(colMeans(X)), item_sd = num(apply(X, 2, sd)),
    item_rest_r = num(sapply(seq_len(k), function(j) cor(X[, j], rowSums(X[, -j, drop = FALSE])))),
    alpha_if_deleted = num(sapply(seq_len(k), function(j) if (k == 2) NA else alpha_of(X[, -j, drop = FALSE]))),
    lower = 1 - (1 - alpha) * qf(1 - tail, df1, df2), upper = 1 - (1 - alpha) * qf(tail, df1, df2))
}

likert <- function(n, k, loading, noise, reversed = integer(0)) {
  trait <- rnorm(n)
  X <- sapply(seq_len(k), function(j) {
    sign <- if (j %in% reversed) -1 else 1
    v <- 3 + sign * loading[j] * trait + rnorm(n, 0, noise)
    pmin(5, pmax(1, round(v)))
  })
  matrix(X, n, k)
}

set.seed(20261112)
X1 <- likert(40, 5, c(1.0, 0.9, 0.8, 0.7, 0.6), 0.8)
X2 <- likert(25, 2, c(1.0, 0.8), 0.7)
X3 <- likert(30, 4, c(1.0, 0.9, 0.8, 0.9), 0.7, reversed = 4)
X4 <- likert(12, 8, c(1, 1, 0.9, 0.9, 0.8, 0.8, 0.7, 0.7), 0.9)
X5 <- likert(60, 3, c(0.4, 0.3, 0.2), 1.0)

result <- list(cases = list(
  case("five_items", X1), case("two_items", X2), case("one_item_scored_the_other_way", X3),
  case("eight_items_few_cases", X4), case("weak_scale_ninety_percent", X5, 0.90)))

path <- file.path(script_dir(), "scale_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
