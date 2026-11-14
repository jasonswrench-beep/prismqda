# Reference values for binary logistic regression, computed by base R's glm (only jsonlite is needed, to write the file).
#
# Run with:  Rscript make_logistic_reference.R
# It writes logistic_reference.json next to this script. tests/logistic_reference.rs requires PrismQDA's logistic regression to
# match these values.
#
# What comes from an R function and what is written out in plain R:
#   coefficients, standard errors, z, p        summary(glm)$coefficients
#   Wald intervals                             confint.default(glm)
#   profile-likelihood intervals               written out: the value of the coefficient at which the deviance, with that
#                                              coefficient held fixed (glm.fit with an offset), has risen by qchisq(level, 1);
#                                              found by uniroot to 1e-12. confint(glm) is R's own profile interval, which it reads
#                                              off a spline through a grid of points, so it agrees with the exact one only to about
#                                              three digits; it is kept here as a loose check (confint_r).
#   deviance, null deviance, AIC, BIC          deviance(glm), glm$null.deviance, AIC(glm), BIC(glm)
#   likelihood-ratio test of the model         pchisq of the difference of the deviances
#   leaving a predictor (or a block) out       the difference of the deviances of two glm fits
#   change between nested models               anova(m1, m2, test = "Chisq")
#   McFadden, Cox-Snell, Nagelkerke            written out from the log likelihoods
#   Hosmer-Lemeshow                            written out as ResourceSelection::hoslem.test does it
#   classification table at .5                 written out
#   area under the ROC curve                   written out from the ranks of the fitted probabilities
#   case by case                               fitted, hatvalues, cooks.distance, residuals(type = pearson / deviance), rstandard
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

num <- function(v) as.numeric(unname(v))
ctl <- glm.control(epsilon = 1e-14, maxit = 100)

hoslem <- function(y, yhat, g = 10) {
  qq <- unique(quantile(yhat, probs = seq(0, 1, 1 / g)))
  cutyhat <- cut(yhat, breaks = qq, include.lowest = TRUE)
  observed <- xtabs(cbind(y0 = 1 - y, y1 = y) ~ cutyhat)
  expected <- xtabs(cbind(yhat0 = 1 - yhat, yhat1 = yhat) ~ cutyhat)
  C <- sum((observed - expected)^2 / expected)
  list(chi2 = C, df = nrow(expected) - 2, p = pchisq(C, nrow(expected) - 2, lower.tail = FALSE),
       n = num(rowSums(observed)), observed = num(observed[, "y1"]), expected = num(expected[, "yhat1"]))
}

auc_of <- function(y, p) {
  r <- rank(p)
  n1 <- sum(y == 1); n0 <- sum(y == 0)
  (sum(r[y == 1]) - n1 * (n1 + 1) / 2) / (n1 * n0)
}

profile_end <- function(X, y, j, est, se, rise, dir) {
  g <- function(b) glm.fit(X[, -j, drop = FALSE], y, offset = b * X[, j], family = binomial(), control = ctl)$deviance - (fit_dev + rise)
  step <- se
  for (i in 1:40) {
    far <- est + dir * step
    if (g(far) > 0) break
    step <- step * 2
  }
  lo <- min(est, far); hi <- max(est, far)
  uniroot(g, c(lo, hi), tol = 1e-13, maxiter = 500)$root
}

expected_for <- function(y, xs, blocks, level = 0.95) {
  d <- as.data.frame(xs); names(d) <- paste0("x", seq_along(xs)); d$y <- y
  k <- length(xs); n <- length(y)
  f <- as.formula(paste("y ~", paste(names(d)[seq_len(k)], collapse = " + ")))
  # Refit starting from the maximum: R takes the weights behind the standard errors from the iterate before the last one, so a fit
  # that starts at the maximum has them exact (otherwise they are off in the eighth digit).
  m <- glm(f, data = d, family = binomial, control = ctl, start = coef(glm(f, data = d, family = binomial, control = ctl)))
  s <- summary(m)$coefficients
  X <- model.matrix(m)
  fit_dev <<- deviance(m)
  rise <- qchisq(level, 1)
  cd <- confint.default(m, level = level)
  ll <- as.numeric(logLik(m)); ll0 <- -m$null.deviance / 2
  fitted_p <- num(fitted(m))
  pr <- num(residuals(m, type = "pearson")); dr <- num(residuals(m, type = "deviance"))
  est <- num(coef(m)); se <- num(s[, 2])
  prof_lo <- c(NA, sapply(2:(k + 1), function(j) profile_end(X, y, j, est[j], se[j], rise, -1)))
  prof_hi <- c(NA, sapply(2:(k + 1), function(j) profile_end(X, y, j, est[j], se[j], rise, 1)))
  cr <- suppressMessages(confint(m, level = level))
  blk <- lapply(blocks, function(b) {
    keep <- setdiff(seq_len(k), b)
    f0 <- if (length(keep)) as.formula(paste("y ~", paste(names(d)[keep], collapse = " + "))) else as.formula("y ~ 1")
    m0 <- glm(f0, data = d, family = binomial, control = ctl)
    chi <- deviance(m0) - deviance(m)
    list(columns = b, chi2 = chi, df = length(b), p = pchisq(chi, length(b), lower.tail = FALSE))
  })
  cl <- table(factor(y, levels = c(0, 1)), factor(as.integer(fitted_p >= 0.5), levels = c(0, 1)))
  list(
    estimate = est, se = se, z = num(s[, 3]), p = num(s[, 4]),
    ci_lo = num(cd[, 1]), ci_hi = num(cd[, 2]),
    profile_lo = prof_lo, profile_hi = prof_hi,
    confint_r_lo = num(cr[, 1]), confint_r_hi = num(cr[, 2]),
    deviance = deviance(m), null_deviance = m$null.deviance, log_likelihood = ll, aic = AIC(m), bic = BIC(m),
    lr_chi2 = m$null.deviance - deviance(m), lr_df = k, lr_p = pchisq(m$null.deviance - deviance(m), k, lower.tail = FALSE),
    mcfadden = 1 - ll / ll0,
    cox_snell = 1 - exp((2 / n) * (ll0 - ll)),
    nagelkerke = (1 - exp((2 / n) * (ll0 - ll))) / (1 - exp((2 / n) * ll0)),
    hosmer_lemeshow = hoslem(y, fitted_p),
    tp = cl["1", "1"], fp = cl["0", "1"], tn = cl["0", "0"], fn = cl["1", "0"],
    auc = auc_of(y, fitted_p),
    fitted = fitted_p, leverage = num(hatvalues(m)), cooks = num(cooks.distance(m)),
    pearson = pr, deviance_residuals = dr, standardized = num(rstandard(m)),
    blocks = blk,
    events = sum(y == 1))
}

change_between <- function(y, xs, upto_small, upto_big) {
  d <- as.data.frame(xs); names(d) <- paste0("x", seq_along(xs)); d$y <- y
  fm <- function(u) as.formula(paste("y ~", paste(names(d)[seq_len(u)], collapse = " + ")))
  m1 <- glm(fm(upto_small), data = d, family = binomial, control = ctl)
  m2 <- glm(fm(upto_big), data = d, family = binomial, control = ctl)
  a <- anova(m1, m2, test = "Chisq")
  list(small = upto_small, big = upto_big, chi2 = a$Deviance[2], df = a$Df[2], p = a$`Pr(>Chi)`[2])
}

case <- function(name, y, xs, blocks = list(), changes = list()) {
  list(name = name, y = y, x = unname(xs), blocks = blocks, expected = expected_for(y, xs, blocks), changes = changes)
}

inv_logit <- function(z) 1 / (1 + exp(-z))
set.seed(2025)

# 1. One number predicting; 60 cases.
x1 <- round(rnorm(60, 50, 10), 1)
y1 <- rbinom(60, 1, inv_logit(-5 + 0.1 * x1))

# 2. A number and a category with three values (dummy columns against the first); 120 cases.
a <- round(rnorm(120, 30, 8), 1)
g <- sample(c("A", "B", "C"), 120, replace = TRUE)
gB <- as.numeric(g == "B"); gC <- as.numeric(g == "C")
y2 <- rbinom(120, 1, inv_logit(-2 + 0.07 * a + 0.9 * gB - 0.6 * gC))

# 3. Four predictors, entered in two steps; 200 cases.
p1 <- round(rnorm(200, 0, 1), 2); p2 <- round(rnorm(200, 5, 2), 2); p3 <- round(0.5 * p1 + rnorm(200), 2); p4 <- rbinom(200, 1, 0.4)
y3 <- rbinom(200, 1, inv_logit(-0.5 + 0.8 * p1 - 0.2 * p2 + 0.4 * p3 + 0.7 * p4))

# 4. A small sample (28 cases), where the Wald and profile intervals differ most.
x4 <- round(rnorm(28, 10, 3), 1)
y4 <- rbinom(28, 1, inv_logit(-6 + 0.5 * x4))

# 5. A category only (many tied predicted probabilities, so tied ranks and repeated break points); 90 cases.
h <- sample(c("low", "mid", "high", "top"), 90, replace = TRUE)
hm <- as.numeric(h == "mid"); hh <- as.numeric(h == "high"); ht <- as.numeric(h == "top")
y5 <- rbinom(90, 1, inv_logit(-1 + 0.6 * hm + 1.0 * hh + 1.6 * ht))

out <- list(
  models = list(
    case("one number", y1, list(x1)),
    case("a number and a category of three", y2, list(a, gB, gC), blocks = list(1, c(2, 3))),
    case("four predictors in two steps", y3, list(p1, p2, p3, p4), blocks = list(1, 2, 3, 4),
         changes = list(change_between(y3, list(p1, p2, p3, p4), 2, 4), change_between(y3, list(p1, p2, p3, p4), 1, 2))),
    case("a small sample", y4, list(x4)),
    case("a category with ties", y5, list(hm, hh, ht), blocks = list(c(1, 2, 3)))
  )
)

args <- commandArgs(trailingOnly = FALSE)
hit <- grep("^--file=", args, value = TRUE)
dir <- if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null"), file.path(dir, "logistic_reference.json"))
cat("wrote", file.path(dir, "logistic_reference.json"), "\n")
