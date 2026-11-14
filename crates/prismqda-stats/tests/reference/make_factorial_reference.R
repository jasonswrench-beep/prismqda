# Reference values for the two-way analysis of variance (type III sums of squares), computed by base R (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_factorial_reference.R
# It writes factorial_reference.json next to this script. tests/factorial_reference.rs requires PrismQDA's analysis to match these values.
#
# What comes from an R function and what is written out in plain R:
#   sums of squares, F, p of each effect     drop1(lm(y ~ A * B, contrasts = contr.sum), scope = ~ A + B + A:B, test = "F"): each effect is
#                                            dropped from the full model while the others stay (type III, effects that add to zero)
#   error sum of squares and degrees         the <none> row of that table, and fit$df.residual
#   R squared, adjusted, total sum of squares  summary(fit), and written out: sum((y - mean(y))^2)
#   partial eta squared                      written out: SS(effect) / (SS(effect) + SS(error))
#   cell and margin means, sds, counts      tapply
#   Levene's test (about the median)         written out: the absolute deviation from the cell median, then anova(lm(deviation ~ cell))
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

case <- function(name, y, a, b) {
  A <- factor(a); B <- factor(b)
  fit <- lm(y ~ A * B, contrasts = list(A = "contr.sum", B = "contr.sum"))
  d1 <- drop1(fit, scope = ~ A + B + A:B, test = "F")
  rows <- 2:4
  ss <- num(d1[["Sum of Sq"]][rows]); df <- num(d1$Df[rows])
  rss <- num(d1$RSS[1]); dfe <- fit$df.residual
  s <- summary(fit)
  cell <- interaction(A, B, lex.order = FALSE)
  # the cells in the order the analysis lists them: first factor's level, then the second's
  cellf <- factor(paste(a, b), levels = paste(rep(sort(unique(a)), each = length(unique(b))), rep(sort(unique(b)), times = length(unique(a)))))
  dev <- abs(y - ave(y, cellf, FUN = median))
  lev <- anova(lm(dev ~ cellf))
  list(
    name = name, y = num(y), a = as.integer(a), b = as.integer(b),
    ss = ss, df = df, f = num(d1[["F value"]][rows]), p = num(d1[["Pr(>F)"]][rows]),
    partial_eta_squared = ss / (ss + rss),
    error_ss = rss, error_df = dfe, error_ms = rss / dfe,
    total_ss = sum((y - mean(y))^2), r_squared = s$r.squared, adj_r_squared = s$adj.r.squared,
    cell_n = as.integer(tapply(y, cellf, length)), cell_mean = num(tapply(y, cellf, mean)), cell_sd = num(tapply(y, cellf, sd)),
    margin_a_n = as.integer(tapply(y, A, length)), margin_a_mean = num(tapply(y, A, mean)),
    margin_b_n = as.integer(tapply(y, B, length)), margin_b_mean = num(tapply(y, B, mean)),
    levene_f = lev[["F value"]][1], levene_df1 = lev$Df[1], levene_df2 = lev$Df[2], levene_p = lev[["Pr(>F)"]][1],
    residuals = num(resid(fit)))
}

design <- function(counts) {
  # counts[i, j] cases in the cell of level i of the first factor and level j of the second
  a <- integer(0); b <- integer(0)
  for (i in seq_len(nrow(counts))) for (j in seq_len(ncol(counts))) { a <- c(a, rep(i - 1L, counts[i, j])); b <- c(b, rep(j - 1L, counts[i, j])) }
  list(a = a, b = b)
}

set.seed(20261112)
d1 <- design(matrix(6, 2, 3))
y1 <- round(50 + c(0, 4, 9)[d1$b + 1] + c(0, 3)[d1$a + 1] + rnorm(length(d1$a), 0, 6), 1)

d2 <- design(matrix(c(3, 8, 5, 4, 7, 6), 3, 2))
y2 <- round(20 + c(0, 2, 5)[d2$a + 1] + c(0, 6)[d2$b + 1] + 0.5 * d2$a * d2$b + rnorm(length(d2$a), 0, 3), 1)

d3 <- design(matrix(5, 2, 2))
y3 <- round(30 + 8 * (d3$a == d3$b) + rnorm(length(d3$a), 0, 2.5), 1)

d4 <- design(matrix(c(4, 5, 6, 7, 3, 8, 5, 6, 4), 3, 3))
y4 <- round(100 + c(0, 5, 12)[d4$a + 1] * c(1, 0.5, 2)[d4$b + 1] + rnorm(length(d4$a), 0, 7), 1)

d5 <- design(matrix(c(2, 2, 3, 2), 2, 2))
y5 <- round(10 + c(0, 3)[d5$a + 1] + c(0, 1)[d5$b + 1] + rnorm(length(d5$a), 0, 1.2), 1)

result <- list(cases = list(
  case("balanced_two_by_three", y1, d1$a, d1$b),
  case("unbalanced_three_by_two", y2, d2$a, d2$b),
  case("crossover_interaction", y3, d3$a, d3$b),
  case("unbalanced_three_by_three", y4, d4$a, d4$b),
  case("small_cells", y5, d5$a, d5$b)))

path <- file.path(script_dir(), "factorial_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
