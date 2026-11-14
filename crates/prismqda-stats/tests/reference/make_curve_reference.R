# Reference values for curve fits, computed by base R's lm (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_curve_reference.R
# It writes curve_reference.json next to this script. tests/curve_reference.rs requires PrismQDA's curve fits to match these values.
#
# Each curve is a straight-line regression of a transformed outcome on transformed predictors, so everything comes from lm:
#   linear        lm(y ~ x)                          logarithmic   lm(y ~ log(x))
#   inverse       lm(y ~ I(1/x))                     quadratic     lm(y ~ x + I(x^2))
#   cubic         lm(y ~ x + I(x^2) + I(x^3))        power         lm(log(y) ~ log(x))
#   exponential   lm(log(y) ~ x)                     S curve       lm(log(y) ~ I(1/x))
# What is written out in plain R: the R squared and root mean squared error on the original scale of the outcome, from the curve's
# predictions (exp of the fitted values for the three curves fitted on the logarithm of the outcome).
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

num <- function(v) as.numeric(unname(v))

formulas <- list(
  linear = function(d) lm(y ~ x, data = d),
  logarithmic = function(d) lm(y ~ log(x), data = d),
  inverse = function(d) lm(y ~ I(1 / x), data = d),
  quadratic = function(d) lm(y ~ x + I(x^2), data = d),
  cubic = function(d) lm(y ~ x + I(x^2) + I(x^3), data = d),
  power = function(d) lm(log(y) ~ log(x), data = d),
  exponential = function(d) lm(log(y) ~ x, data = d),
  s_curve = function(d) lm(log(y) ~ I(1 / x), data = d)
)
logged <- c("power", "exponential", "s_curve")

expected_for <- function(kind, x, y, level = 0.95) {
  d <- data.frame(x = x, y = y)
  m <- formulas[[kind]](d)
  s <- summary(m); ci <- confint(m, level = level)
  fit <- num(fitted(m))
  yhat <- if (kind %in% logged) exp(fit) else fit
  sse <- sum((y - yhat)^2); sst <- sum((y - mean(y))^2)
  list(
    estimate = num(s$coefficients[, 1]), se = num(s$coefficients[, 2]), t = num(s$coefficients[, 3]), p = num(s$coefficients[, 4]),
    ci_lo = num(ci[, 1]), ci_hi = num(ci[, 2]),
    r_squared = s$r.squared, adj_r_squared = s$adj.r.squared, f = num(s$fstatistic[1]), df1 = num(s$fstatistic[2]), df2 = num(s$fstatistic[3]),
    f_p = pf(s$fstatistic[1], s$fstatistic[2], s$fstatistic[3], lower.tail = FALSE), sigma = s$sigma,
    r_squared_original = 1 - sse / sst, rmse_original = sqrt(sse / length(y)),
    predicted = num(yhat))
}

case <- function(name, x, y) list(name = name, x = x, y = y, kinds = lapply(setNames(names(formulas), names(formulas)), function(k) expected_for(k, x, y)))

set.seed(2026)
x1 <- round(runif(30, 1, 12), 2); y1 <- round(3 * exp(0.22 * x1) * exp(rnorm(30, 0, 0.15)), 2)
x2 <- round(seq(0.5, 15, length.out = 25) + rnorm(25, 0, 0.2), 2); y2 <- round(8 + 1.4 * x2 - 0.09 * x2^2 + rnorm(25, 0, 0.9), 2)
x3 <- round(runif(40, 0.2, 9), 2); y3 <- round(20 / (1 + 0.4 * x3) + rnorm(40, 0, 0.5) + 2, 2)

out <- list(models = list(case("growth that speeds up", x1, y1), case("rises then levels off", x2, y2), case("falls toward a floor", x3, y3)))

args <- commandArgs(trailingOnly = FALSE)
hit <- grep("^--file=", args, value = TRUE)
dir <- if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null"), file.path(dir, "curve_reference.json"))
cat("wrote", file.path(dir, "curve_reference.json"), "\n")
