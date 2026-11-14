# Reference values for the agreement coefficients, computed by R packages.
#
# Run with:  Rscript make_agreement_reference.R
# It writes agreement_reference.json next to this script. The Rust tests read that file and require
# PrismQDA's coefficients to match these values.
#
# Packages (from CRAN):
#   irr     0.85  kappa2 (Cohen), kappam.fleiss (Fleiss), kripp.alpha (Krippendorff), agree
#   irrCAC  1.4   fleiss.kappa.raw (Fleiss' generalized kappa, which pools the coders' marginals the way
#                 Scott's pi does, and equals Scott's pi for two coders), bp.coeff.raw (Brennan-Prediger),
#                 gwet.ac1.raw (Gwet's AC1), krippen.alpha.raw (Krippendorff), pa.coeff.raw (percent agreement)
#   jsonlite      writes the file with full numeric precision
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages({
  library(irr)
  library(irrCAC)
  library(jsonlite)
})

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}

# A rating matrix: one row per unit, one column per coder, categories are 1..q, NA is "not rated".
make_ratings <- function(seed, n_units, n_coders, q, agree_prob, miss_prob = 0, prevalence = NULL) {
  set.seed(seed)
  p <- if (is.null(prevalence)) rep(1 / q, q) else prevalence
  truth <- sample.int(q, n_units, replace = TRUE, prob = p)
  m <- matrix(NA_integer_, n_units, n_coders)
  for (j in seq_len(n_coders)) {
    same <- runif(n_units) < agree_prob
    m[, j] <- ifelse(same, truth, sample.int(q, n_units, replace = TRUE, prob = p))
  }
  if (miss_prob > 0) m[matrix(runif(n_units * n_coders) < miss_prob, n_units, n_coders)] <- NA_integer_
  m
}

datasets <- list(
  two_coders_five_codes = list(m = make_ratings(1, 40, 2, 5, 0.75), q = 5),
  # The kappa paradox: 91 percent agreement, yet kappa is only about 0.52, because almost every unit is in
  # category 1. Built from a 2 x 2 table (both 1, coder A only 1, coder B only 1, both 2).
  kappa_paradox = list(m = {
    rows <- c(rep(list(c(1L, 1L)), 85), rep(list(c(1L, 2L)), 4), rep(list(c(2L, 1L)), 5), rep(list(c(2L, 2L)), 6))
    do.call(rbind, rows)
  }, q = 2),
  three_coders_complete = list(m = make_ratings(3, 60, 3, 4, 0.65), q = 4),
  four_coders_with_gaps = list(m = make_ratings(4, 80, 4, 6, 0.6, miss_prob = 0.15), q = 6),
  three_coders_singletons = {
    m <- make_ratings(5, 50, 3, 3, 0.7, miss_prob = 0.3)
    m[1:4, ] <- NA_integer_         # four units nobody rated
    m[5:9, 2:3] <- NA_integer_      # five units rated by one coder only
    list(m = m, q = 3)
  },
  perfect_agreement = list(m = {
    set.seed(7); t <- sample.int(4, 30, replace = TRUE); cbind(t, t, t)
  }, q = 4),
  chance_level = list(m = {
    set.seed(8); cbind(sample.int(3, 90, replace = TRUE), sample.int(3, 90, replace = TRUE))
  }, q = 3)
)


# The same coefficients written out in plain R from their published definitions, at full precision.
# irrCAC rounds what it returns to five decimals, so these let the Rust tests match to many more digits.
# The script stops if they disagree with the packages beyond that rounding, which proves they are the
# same definitions. A unit needs at least two ratings to count towards agreement; the chance-agreement
# terms use every unit that has at least one rating.
manual_coefficients <- function(m, q) {
  counts <- t(apply(m, 1, function(row) tabulate(row[!is.na(row)], nbins = q)))
  r_i <- rowSums(counts)
  two <- r_i >= 2
  pa <- mean(rowSums(counts[two, , drop = FALSE] * (counts[two, , drop = FALSE] - 1)) / (r_i[two] * (r_i[two] - 1)))
  one <- r_i >= 1
  pik <- colMeans(counts[one, , drop = FALSE] / r_i[one])
  list(
    pa = pa,
    scott_pi = (pa - sum(pik^2)) / (1 - sum(pik^2)),
    brennan_prediger = (pa - 1 / q) / (1 - 1 / q),
    gwet_ac1 = (pa - sum(pik * (1 - pik)) / (q - 1)) / (1 - sum(pik * (1 - pik)) / (q - 1))
  )
}

# Krippendorff's alpha for nominal categories from the coincidence matrix (Krippendorff 2011), in plain R.
manual_alpha <- function(m, q) {
  o <- matrix(0, q, q)
  for (i in seq_len(nrow(m))) {
    v <- m[i, !is.na(m[i, ])]
    k <- length(v)
    if (k < 2) next
    cnt <- tabulate(v, nbins = q)
    o <- o + (outer(cnt, cnt) - diag(cnt)) / (k - 1)
  }
  nc <- rowSums(o)
  n <- sum(nc)
  d_obs <- (sum(o) - sum(diag(o))) / n
  d_exp <- (sum(outer(nc, nc)) - sum(nc^2)) / (n * (n - 1))
  1 - d_obs / d_exp
}

agree_within <- function(a, b, what, tol = 6e-6) {
  if (!is.null(a) && !is.null(b) && abs(a - b) > tol) stop(sprintf("manual %s %.9f disagrees with the package value %.9f", what, a, b))
}

pick <- function(x) if (is.null(x) || length(x) == 0 || is.na(x)) NULL else as.numeric(x)

expected_for <- function(m, q) {
  # irrCAC wants the ratings as a data frame of category labels, with NA for missing.
  df <- as.data.frame(m)
  labels <- as.character(seq_len(q))
  df[] <- lapply(df, function(col) ifelse(is.na(col), NA_character_, as.character(col)))
  out <- list()

  bp <- bp.coeff.raw(df, categ.labels = labels)$est
  out$percent_agreement <- pick(bp$pa)
  out$brennan_prediger <- pick(bp$coeff.val)

  ac1 <- gwet.ac1.raw(df, categ.labels = labels)$est
  out$gwet_ac1 <- pick(ac1$coeff.val)

  pi <- fleiss.kappa.raw(df, categ.labels = labels)$est
  out$scott_pi <- pick(pi$coeff.val)

  out$percent_agreement_irrcac <- pick(pa.coeff.raw(df, categ.labels = labels)$est$coeff.val)

  ka <- krippen.alpha.raw(df, categ.labels = labels)$est
  out$krippendorff_alpha_irrcac <- pick(ka$coeff.val)

  # Krippendorff's alpha from the irr package, with coders in rows as it expects.
  kr <- tryCatch(kripp.alpha(t(m), method = "nominal")$value, error = function(e) NA)
  out$krippendorff_alpha <- pick(kr)
  out$manual_krippendorff_alpha <- manual_alpha(m, q)
  agree_within(out$manual_krippendorff_alpha, out$krippendorff_alpha_irrcac, "Krippendorff alpha against irrCAC")
  # On one of the datasets below, irr 0.85 disagrees with the definition and with irrCAC. Record where,
  # so the Rust tests can hold PrismQDA to the definition and still report the package difference.
  out$irr_alpha_differs_from_definition <- !is.null(out$krippendorff_alpha) &&
    abs(out$krippendorff_alpha - out$manual_krippendorff_alpha) > 1e-6

  if (any(rowSums(!is.na(m)) >= 2)) {
    man <- manual_coefficients(m, q)
    out$manual_percent_agreement <- man$pa
    out$manual_scott_pi <- man$scott_pi
    out$manual_brennan_prediger <- man$brennan_prediger
    out$manual_gwet_ac1 <- man$gwet_ac1
    agree_within(man$pa, out$percent_agreement, "percent agreement")
    agree_within(man$brennan_prediger, out$brennan_prediger, "Brennan-Prediger")
    # AC1 and generalized pi are only defined by the packages when every unit has a rating.
    if (all(rowSums(!is.na(m)) >= 1)) {
      agree_within(man$gwet_ac1, out$gwet_ac1, "Gwet AC1")
      agree_within(man$scott_pi, out$scott_pi, "Scott pi")
    }
  }

  if (ncol(m) == 2) {
    both <- m[complete.cases(m), , drop = FALSE]
    out$cohen_kappa <- pick(kappa2(both)$value)
    out$percent_agreement_simple <- pick(agree(both)$value / 100)
  }

  full <- m[complete.cases(m), , drop = FALSE]
  if (nrow(full) >= 2 && ncol(m) >= 2) {
    fk <- tryCatch(kappam.fleiss(full)$value, error = function(e) NA)
    out$fleiss_kappa <- pick(fk)
    out$fleiss_units <- nrow(full)
  }
  out
}

result <- lapply(names(datasets), function(name) {
  d <- datasets[[name]]
  list(name = name, categories = d$q, ratings = unname(d$m), expected = expected_for(d$m, d$q))
})

path <- file.path(script_dir(), "agreement_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "| irr", as.character(packageVersion("irr")), "| irrCAC", as.character(packageVersion("irrCAC")), "\n")
