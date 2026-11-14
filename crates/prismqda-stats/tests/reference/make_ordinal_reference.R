# Reference values for the measures of association between two ordered categories (Goodman and Kruskal's gamma, Kendall's tau-b,
# Stuart's tau-c, and Somers' d), computed by base R only (no packages except jsonlite, to write the file).
#
# Run with:  Rscript make_ordinal_reference.R
# It writes ordinal_reference.json next to this script. tests/ordinal_reference.rs requires PrismQDA's functions to match.
#
# Base R has no function for these, so the point estimates are found the plain way, by looking at every pair of cases, and
# Kendall's tau-b is also checked against R's own cor(method = "kendall") on the expanded data. The standard errors are the large-sample
# formulas of Agresti (2002, section 2.4) and the SAS/STAT documentation. A formula can be mistyped, so the script also checks
# each standard error against the spread of the statistic over 4,000 tables drawn from the same proportions (a parametric
# bootstrap), and stops if they differ by more than 8 percent (a formula with a mistake in it is off by far more; the large-sample
# formulas themselves are a little off when a statistic is near its limit, as gamma is in the two by two table, where it is 6 percent off
# the bootstrap and exactly Yule's closed form below).

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}

tables <- list(
  # Agresti (2002), Table 2.8: income by job satisfaction. gamma = .221 in the book.
  list(name = "Agresti table 2.8, income by job satisfaction", t = rbind(c(1, 2, 1, 0), c(3, 3, 6, 1), c(10, 10, 14, 9), c(6, 7, 12, 11))),
  list(name = "two by two", t = rbind(c(35, 12), c(9, 28))),
  list(name = "two by three", t = rbind(c(20, 14, 6), c(8, 15, 22))),
  list(name = "three by five", t = rbind(c(5, 9, 14, 8, 3), c(2, 7, 12, 15, 9), c(1, 2, 6, 13, 18))),
  list(name = "five by three", t = rbind(c(12, 5, 1), c(9, 8, 3), c(6, 11, 7), c(2, 9, 14), c(1, 4, 16))),
  list(name = "four by four, strong", t = rbind(c(30, 8, 2, 0), c(9, 25, 7, 1), c(2, 10, 28, 6), c(0, 3, 11, 32))),
  list(name = "three by three, weak", t = rbind(c(10, 11, 9), c(12, 9, 10), c(9, 10, 12))),
  list(name = "reversed, negative", t = rbind(c(2, 6, 18), c(7, 15, 8), c(20, 9, 3))),
  list(name = "with a sparse cell", t = rbind(c(14, 3, 0), c(6, 11, 2), c(1, 5, 9)))
)

# For every cell: the number of cases concordant with it (above and left, below and right), and discordant.
cd_matrices <- function(t) {
  r <- nrow(t); k <- ncol(t)
  A <- D <- matrix(0, r, k)
  for (i in 1:r) for (j in 1:k) {
    a <- 0; d <- 0
    for (u in 1:r) for (v in 1:k) {
      if ((u > i && v > j) || (u < i && v < j)) a <- a + t[u, v]
      if ((u > i && v < j) || (u < i && v > j)) d <- d + t[u, v]
    }
    A[i, j] <- a; D[i, j] <- d
  }
  list(A = A, D = D)
}

measures <- function(t) {
  r <- nrow(t); k <- ncol(t); n <- sum(t); ri <- rowSums(t); cj <- colSums(t)
  cd <- cd_matrices(t); A <- cd$A; D <- cd$D; d <- A - D
  P <- sum(t * A); Q <- sum(t * D)         # pairs counted twice, as SAS does
  wr <- n^2 - sum(ri^2); wc <- n^2 - sum(cj^2); w <- sqrt(wr * wc)
  m <- min(r, k)
  gamma <- (P - Q) / (P + Q)
  ase_gamma <- 4 / (P + Q)^2 * sqrt(sum(t * (Q * A - P * D)^2))
  ase0_gamma <- 2 / (P + Q) * sqrt(sum(t * d^2) - (P - Q)^2 / n)
  taub <- (P - Q) / w
  v <- outer(ri, rep(1, k)) * wc + outer(rep(1, r), cj) * wr
  ase_taub <- 1 / w^2 * sqrt(sum(t * (2 * w * d + taub * v)^2) - n^3 * taub^2 * (wr + wc)^2)
  ase0_taub <- 2 / w * sqrt(sum(t * d^2) - (P - Q)^2 / n)
  tauc <- (P - Q) * m / (n^2 * (m - 1))   # P and Q count each pair twice here, so this is 2 m (pairs concordant - discordant) / (n^2 (m - 1))
  ase_tauc <- 2 * m / ((m - 1) * n^2) * sqrt(sum(t * d^2) - (P - Q)^2 / n)
  ase0_tauc <- ase_tauc
  # Somers' d with the columns as the dependent variable (rows independent), and the reverse.
  d_cr <- (P - Q) / wr
  ase_dcr <- 2 / wr^2 * sqrt(sum(t * (wr * d - (P - Q) * (n - outer(ri, rep(1, k))))^2))
  ase0_dcr <- 2 / wr * sqrt(sum(t * d^2) - (P - Q)^2 / n)
  d_rc <- (P - Q) / wc
  ase_drc <- 2 / wc^2 * sqrt(sum(t * (wc * d - (P - Q) * (n - outer(rep(1, r), cj)))^2))
  ase0_drc <- 2 / wc * sqrt(sum(t * d^2) - (P - Q)^2 / n)
  d_sym <- (P - Q) / ((wr + wc) / 2)
  z <- function(est, ase0) est / ase0
  p2 <- function(zz) 2 * pnorm(-abs(zz))
  list(n = n, concordant = P / 2, discordant = Q / 2,
       gamma = gamma, gamma_ase = ase_gamma, gamma_ase0 = ase0_gamma, gamma_z = z(gamma, ase0_gamma), gamma_p = p2(z(gamma, ase0_gamma)),
       tau_b = taub, tau_b_ase = ase_taub, tau_b_ase0 = ase0_taub, tau_b_z = z(taub, ase0_taub), tau_b_p = p2(z(taub, ase0_taub)),
       tau_c = tauc, tau_c_ase = ase_tauc, tau_c_ase0 = ase0_tauc, tau_c_z = z(tauc, ase0_tauc), tau_c_p = p2(z(tauc, ase0_tauc)),
       somers_columns_dependent = d_cr, somers_cd_ase = ase_dcr, somers_cd_ase0 = ase0_dcr, somers_cd_z = z(d_cr, ase0_dcr), somers_cd_p = p2(z(d_cr, ase0_dcr)),
       somers_rows_dependent = d_rc, somers_rd_ase = ase_drc, somers_rd_ase0 = ase0_drc, somers_rd_z = z(d_rc, ase0_drc), somers_rd_p = p2(z(d_rc, ase0_drc)),
       somers_symmetric = d_sym)
}

# The plain way: every pair of cases, from the expanded data.
by_pairs <- function(t) {
  idx <- which(t > 0, arr.ind = TRUE)
  x <- rep(idx[, 1], t[idx]); y <- rep(idx[, 2], t[idx]); n <- length(x)
  P <- Q <- Tx <- Ty <- 0
  for (a in 1:(n - 1)) {
    dx <- sign(x[a] - x[(a + 1):n]); dy <- sign(y[a] - y[(a + 1):n])
    P <- P + sum(dx * dy > 0); Q <- Q + sum(dx * dy < 0)
    Tx <- Tx + sum(dx == 0 & dy != 0); Ty <- Ty + sum(dy == 0 & dx != 0)
  }
  m <- min(dim(t))
  list(x = x, y = y, P = P, Q = Q, gamma = (P - Q) / (P + Q), tau_b = (P - Q) / sqrt((P + Q + Tx) * (P + Q + Ty)),
       tau_c = 2 * m * (P - Q) / (n^2 * (m - 1)), d_cd = (P - Q) / (P + Q + Ty), d_rd = (P - Q) / (P + Q + Tx),
       kendall = suppressWarnings(cor(x, y, method = "kendall")))
}

set.seed(20261108)
out <- lapply(tables, function(tb) {
  t <- tb$t
  m <- measures(t); bp <- by_pairs(t)
  if (all(dim(t) == 2)) {   # Yule's Q: the standard error has a closed form
    q <- (t[1, 1] * t[2, 2] - t[1, 2] * t[2, 1]) / (t[1, 1] * t[2, 2] + t[1, 2] * t[2, 1])
    stopifnot(abs(m$gamma_ase - 0.5 * (1 - q^2) * sqrt(sum(1 / t))) < 1e-12)
  }
  stopifnot(abs(m$concordant - bp$P) < 1e-9, abs(m$discordant - bp$Q) < 1e-9)
  stopifnot(abs(m$gamma - bp$gamma) < 1e-12, abs(m$tau_b - bp$tau_b) < 1e-12, abs(m$tau_b - bp$kendall) < 1e-12, abs(m$tau_c - bp$tau_c) < 1e-12)
  stopifnot(abs(m$somers_columns_dependent - bp$d_cd) < 1e-12, abs(m$somers_rows_dependent - bp$d_rd) < 1e-12)
  # The standard errors against a parametric bootstrap of the table.
  draws <- rmultinomial <- function(B) {
    prob <- as.vector(t) / sum(t)
    sapply(1:B, function(b) {
      tt <- matrix(as.vector(rmultinom(1, sum(t), prob)), nrow(t), ncol(t))
      if (any(rowSums(tt) == 0) || any(colSums(tt) == 0)) return(rep(NA_real_, 5))
      mm <- measures(tt); c(mm$gamma, mm$tau_b, mm$tau_c, mm$somers_columns_dependent, mm$somers_rows_dependent)
    })
  }
  s <- apply(draws(4000), 1, sd, na.rm = TRUE)
  want <- c(m$gamma_ase, m$tau_b_ase, m$tau_c_ase, m$somers_cd_ase, m$somers_rd_ase)
  rel <- abs(s / want - 1)
  cat(sprintf("%-48s standard errors against the bootstrap: worst %.1f%%\n", tb$name, 100 * max(rel)))
  if (max(rel) > 0.08) { print(rbind(bootstrap = s, formula = want, relative = rel)); stop("a standard error does not match the bootstrap for ", tb$name) }
  list(name = tb$name, table = t, expected = m)
})

path <- file.path(script_dir(), "ordinal_reference.json")
write(toJSON(out, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
