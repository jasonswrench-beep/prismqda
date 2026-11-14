# Reference values for linear regression, computed by base R's lm (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_regression_reference.R
# It writes regression_reference.json next to this script. tests/regression_reference.rs requires PrismQDA's regression to
# match these values.
#
# What comes from an R function and what is written out in plain R:
#   coefficients, standard errors, t, p      summary(lm)$coefficients
#   intervals                                confint(lm)
#   R squared, adjusted, F, sigma            summary(lm)
#   case by case                             fitted, residuals, hatvalues, cooks.distance, rstandard, rstudent
#   standardized coefficients                written out: b * sd(x) / sd(y)
#   variance inflation factors               written out: 1 / (1 - R squared of the predictor on the others)
#   Breusch-Pagan (Koenker)                  written out: n times the R squared of the squared residuals on the predictors,
#                                            as lmtest::bptest does with its default studentize = TRUE
#   change between nested models             anova(m1, m2)
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

expected_for <- function(y, xs) {
  d <- as.data.frame(xs); names(d) <- paste0("x", seq_along(xs)); d$y <- y
  f <- as.formula(paste("y ~", paste(names(d)[seq_along(xs)], collapse = " + ")))
  m <- lm(f, data = d); s <- summary(m); ci <- confint(m)
  k <- length(xs); n <- length(y)
  beta <- sapply(seq_len(k), function(j) coef(m)[[j + 1]] * sd(xs[[j]]) / sd(y))
  vif <- sapply(seq_len(k), function(j) {
    if (k == 1) return(1)
    others <- d[, setdiff(names(d)[seq_len(k)], paste0("x", j)), drop = FALSE]
    1 / (1 - summary(lm(d[[paste0("x", j)]] ~ ., data = others))$r.squared)
  })
  e2 <- resid(m)^2
  aux <- summary(lm(as.formula(paste("e2 ~", paste(names(d)[seq_len(k)], collapse = " + "))), data = cbind(d, e2 = e2)))$r.squared
  bp <- n * aux
  list(
    estimate = num(s$coefficients[, 1]), se = num(s$coefficients[, 2]), t = num(s$coefficients[, 3]), p = num(s$coefficients[, 4]),
    ci_lo = num(ci[, 1]), ci_hi = num(ci[, 2]), beta = num(beta), vif = num(vif),
    r_squared = s$r.squared, adj_r_squared = s$adj.r.squared, f = num(s$fstatistic[1]), df1 = num(s$fstatistic[2]), df2 = num(s$fstatistic[3]),
    f_p = pf(s$fstatistic[1], s$fstatistic[2], s$fstatistic[3], lower.tail = FALSE), sigma = s$sigma, rss = sum(resid(m)^2), tss = sum((y - mean(y))^2),
    fitted = num(fitted(m)), residuals = num(resid(m)), leverage = num(hatvalues(m)), cooks = num(cooks.distance(m)),
    standardized = num(rstandard(m)), studentized = num(rstudent(m)),
    bp_statistic = bp, bp_df = k, bp_p = pchisq(bp, k, lower.tail = FALSE))
}

case <- function(name, y, xs) list(name = name, y = y, x = unname(xs), expected = expected_for(y, xs))

set.seed(2024)
x1 <- round(rnorm(30, 10, 3), 2); y1 <- round(2 + 0.7 * x1 + rnorm(30, 0, 2), 2)

a1 <- round(rnorm(60, 20, 5), 2); a2 <- round(0.5 * a1 + rnorm(60, 0, 3), 2); a3 <- round(rnorm(60, 5, 2), 2)
y2 <- round(1 + 0.5 * a1 - 0.8 * a2 + 0.3 * a3 + rnorm(60, 0, 3), 2)

x4 <- c(round(rnorm(24, 10, 2), 2), 30); y4 <- c(round(3 + 0.5 * x4[1:24] + rnorm(24, 0, 1), 2), 5)

s1 <- round(rnorm(9, 5, 2), 2); s2 <- round(rnorm(9, 0, 1), 2); y5 <- round(1 + s1 - 2 * s2 + rnorm(9, 0, 1), 2)

h1 <- round(runif(80, 1, 10), 2); h2 <- round(rnorm(80), 2); y6 <- round(2 + h1 + h2 + rnorm(80, 0, h1 * 0.6), 2)

# A dummy-coded categorical predictor (three groups, the first as reference) with a covariate.
grp <- sample(c("a", "b", "c"), 45, replace = TRUE); cov <- round(rnorm(45, 50, 10), 1)
dB <- as.numeric(grp == "b"); dC <- as.numeric(grp == "c")
y7 <- round(10 + 3 * dB - 2 * dC + 0.2 * cov + rnorm(45, 0, 2), 2)

# Models entered in steps, on the second data set.
steps <- list(m1 = lm(y2 ~ a1), m2 = lm(y2 ~ a1 + a2), m3 = lm(y2 ~ a1 + a2 + a3))
step_info <- lapply(2:3, function(i) {
  a <- anova(steps[[i - 1]], steps[[i]])
  list(from = i - 1, to = i, r2_change = summary(steps[[i]])$r.squared - summary(steps[[i - 1]])$r.squared, f = a$F[2], df1 = a$Df[2], df2 = a$Res.Df[2], p = a[["Pr(>F)"]][2])
})

result <- list(
  models = list(
    case("simple", y1, list(x1)),
    case("three_correlated_predictors", y2, list(a1, a2, a3)),
    case("high_leverage_outlier", y4, list(x4)),
    case("small_two_predictors", y5, list(s1, s2)),
    case("unequal_error_variance", y6, list(h1, h2)),
    case("dummy_coded_group_and_covariate", y7, list(dB, dC, cov))),
  steps = list(y = y2, x = list(a1, a2, a3), changes = step_info)
)

path <- file.path(script_dir(), "regression_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
