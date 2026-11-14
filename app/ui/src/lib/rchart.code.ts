// R code for the charts of a page: each chart is drawn with R's base graphics, and the numbers it is drawn from are printed, the same
// numbers the table beside the chart on the page holds (the bins of a histogram, the five numbers of a box, the points of a normal
// probability plot). The rules are the engine's own (the width of a bar, the quartiles by R's default method, the fences at 1.5
// interquartile ranges), written out here so that what R draws is what the page draws. Nothing in this file is words for the person, so
// it is not audited for language (see scripts/i18n/ast.mjs); the titles and the names on the axes come from the page.

import type { StatSubject } from '../types';
import { modelSetup, q, rn, vec, type Helper, type RPage } from './rbase.code';

export const CHART_HELPERS: Record<string, Helper> = {
  pq_bins: {
    code: `# The edges of the bars of a histogram: a width by Sturges' rule rounded up to 1, 2, 5, or 10 times a power of ten, so the edges are numbers a person would write.
pq_bins <- function(x) {
  x <- x[is.finite(x)]; lo <- min(x); hi <- max(x)
  tidy <- function(v) if (v == 0 || !is.finite(v)) v else { s <- 10^(11 - floor(log10(abs(v)))); round(v * s) / s }
  if (lo == hi) return(c(tidy(lo - 0.5), tidy(lo + 0.5)))
  raw <- (hi - lo) / max(ceiling(log2(length(x))) + 1, 3); mag <- 10^floor(log10(raw)); r <- raw / mag
  width <- mag * (if (r <= 1) 1 else if (r <= 2) 2 else if (r <= 5) 5 else 10)
  start <- tidy(floor(lo / width) * width); edges <- start; i <- 1
  while (edges[length(edges)] < hi) { edges <- c(edges, tidy(start + i * width)); i <- i + 1 }
  edges
}`,
  },
  pq_hist: {
    code: `pq_hist <- function(x, main, xlab, ylab, reference = NULL) {
  x <- x[is.finite(x)]; edges <- pq_bins(x)
  h <- hist(x, breaks = edges, right = TRUE, include.lowest = TRUE, main = main, xlab = xlab, ylab = ylab)
  if (!is.null(reference)) abline(v = reference, lty = 2)
  print(data.frame(from = edges[-length(edges)], to = edges[-1], cases = h$counts))
}`,
    needs: ['pq_bins'],
  },
  pq_bars: {
    code: `pq_bars <- function(x, levels, main, xlab, ylab) {
  tab <- table(factor(x, levels = levels))
  barplot(tab, main = main, xlab = xlab, ylab = ylab)
  print(cbind(cases = tab, percent_of_cases_with_a_value = 100 * tab / sum(tab)))
}`,
  },
  pq_box: {
    code: `# A box for each group: the quartiles by R's default method (type 7), the whiskers to the furthest value within 1.5 interquartile ranges of the box.
pq_box <- function(y, g, main, xlab, ylab) {
  s <- lapply(split(y, g), function(v) {
    qs <- quantile(v, c(0.25, 0.5, 0.75), type = 7, names = FALSE); fence <- 1.5 * (qs[3] - qs[1])
    inside <- v[v >= qs[1] - fence & v <= qs[3] + fence]
    list(stats = c(min(inside), qs, max(inside)), n = length(v), mean = mean(v), out = sort(v[v < qs[1] - fence | v > qs[3] + fence]))
  })
  z <- list(stats = sapply(s, function(a) a$stats), n = sapply(s, function(a) a$n), out = unlist(lapply(s, function(a) a$out), use.names = FALSE),
            group = rep(seq_along(s), sapply(s, function(a) length(a$out))), names = names(s))
  bxp(z, main = main, xlab = xlab, ylab = ylab)
  print(data.frame(n = z$n, lower_whisker = z$stats[1, ], q1 = z$stats[2, ], median = z$stats[3, ], q3 = z$stats[4, ], upper_whisker = z$stats[5, ], mean = sapply(s, function(a) a$mean)))
  for (k in names(s)) if (length(s[[k]]$out) > 0) cat("beyond the whiskers in", k, ":", s[[k]]$out, "\\n")
}`,
  },
  pq_means: {
    code: `# The mean of each group with the interval of that mean (Student's t).
pq_means <- function(y, g, level, main, xlab, ylab) {
  s <- t(sapply(split(y, g), function(v) {
    n <- length(v); half <- if (n >= 2) qt(1 - (1 - level) / 2, n - 1) * sd(v) / sqrt(n) else NA_real_
    c(n = n, mean = mean(v), lower = mean(v) - half, upper = mean(v) + half)
  }))
  k <- nrow(s)
  plot(seq_len(k), s[, "mean"], xaxt = "n", xlim = c(0.5, k + 0.5), ylim = range(s[, c("mean", "lower", "upper")], na.rm = TRUE), pch = 19, main = main, xlab = xlab, ylab = ylab)
  axis(1, at = seq_len(k), labels = rownames(s))
  segments(seq_len(k), s[, "lower"], seq_len(k), s[, "upper"])
  print(s)
}`,
  },
  pq_stacked: {
    code: `# The cases of each row of a table, divided among its columns, as shares of the row.
pq_stacked <- function(tab, main, xlab, ylab) {
  shares <- prop.table(tab, 1); shares[is.nan(shares)] <- 0
  barplot(t(shares), legend.text = colnames(tab), args.legend = list(x = "topright"), main = main, xlab = xlab, ylab = ylab)
  print(tab); print(100 * shares)
}`,
  },
  pq_qq: {
    code: `# A normal probability plot (R's qqnorm) with the line through the quartiles (R's qqline). The points are printed for up to 100 values.
pq_qq <- function(v, main, xlab, ylab) {
  v <- v[is.finite(v)]
  pts <- qqnorm(v, main = main, xlab = xlab, ylab = ylab)
  y13 <- quantile(v, c(0.25, 0.75), type = 7, names = FALSE); x13 <- qnorm(c(0.25, 0.75))
  slope <- (y13[2] - y13[1]) / (x13[2] - x13[1]); abline(y13[1] - slope * x13[1], slope)
  print(c(intercept = y13[1] - slope * x13[1], slope = slope))
  if (length(v) <= 100) print(data.frame(expected_if_normal = sort(pts$x), value = sort(pts$y)))
}`,
  },
  pq_scatter: {
    code: `pq_scatter <- function(x, y, main, xlab, ylab, line = NULL, zero = FALSE) {
  plot(x, y, main = main, xlab = xlab, ylab = ylab)
  if (zero) abline(h = 0, lty = 2)
  if (!is.null(line)) { abline(line[1], line[2]); print(c(intercept = line[1], slope = line[2])) }
}`,
  },
  pq_curves: {
    code: `# The points with each curve that was fitted, drawn along the range of x.
pq_curves <- function(x, y, fits, main, xlab, ylab) {
  plot(x, y, main = main, xlab = xlab, ylab = ylab)
  grid <- seq(min(x), max(x), length.out = 81); kinds <- names(fits)
  for (j in seq_along(kinds)) {
    yy <- predict(fits[[j]], data.frame(x = grid))
    if (kinds[j] %in% c("power", "exponential", "s_curve")) yy <- exp(yy)
    ok <- is.finite(yy); lines(grid[ok], yy[ok], col = j + 1, lty = j)
  }
  legend("topleft", legend = kinds, col = seq_along(kinds) + 1, lty = seq_along(kinds))
}`,
  },
};

/** The charts of a page, drawn and their numbers printed. Empty for a page with none. */
export function chartCode(p: RPage): string[] {
  const out: string[] = [];
  const fit = p.procedure === 'linear_regression' ? modelSetup(p, false).last : '';
  const label = (i: number) => p.vars[i]?.label ?? '';
  /** The R expression of what a chart is about. */
  const subject = (s: StatSubject): string => {
    switch (s.type) {
      case 'differences':
        return 'x - y';
      case 'residuals':
        return `resid(${fit})`;
      case 'fitted':
        return `fitted(${fit})`;
      default:
        if (p.procedure === 'linear_regression') return s.index === 0 ? 'm[["y"]]' : `m[[${q(label(s.index))}]]`;
        if (p.procedure === 'curve_fit') return s.index === 0 ? 'y' : 'x';
        return s.index === 0 ? 'x' : 'y';
    }
  };
  for (const { chart: c, title, names } of p.charts ?? []) {
    const main = q(title);
    const [a = '', b = ''] = names;
    const words = `${main}, ${q(a)}, ${q(b)}`;
    switch (c.kind) {
      case 'histogram':
        out.push(`pq_hist(${subject(c.subject)}, ${words}${c.reference !== null ? `, ${rn(c.reference)}` : ''})`);
        break;
      case 'bars':
        out.push(`pq_bars(x, ${vec(c.categories.map((k) => k.value))}, ${words})`);
        break;
      case 'box':
        out.push(`pq_box(y, g, ${words})`);
        break;
      case 'means':
        out.push(`pq_means(y, g, ${rn(c.level)}, ${words})`);
        break;
      case 'stacked':
        out.push(`pq_stacked(tab, ${words})`);
        break;
      case 'qq':
        out.push(`pq_qq(${subject(c.subject)}, ${words})`);
        break;
      case 'scatter': {
        const line = c.fit ? (p.procedure === 'linear_regression' ? `unname(coef(${fit}))` : 'unname(coef(lm(y ~ x)))') : '';
        out.push(`pq_scatter(${subject(c.x)}, ${subject(c.y)}, ${words}${line ? `, line = ${line}` : ''}${c.zero_line ? ', zero = TRUE' : ''})`);
        break;
      }
      case 'curves':
        out.push(`pq_curves(x, y, fits[c(${c.curves.map((k) => q(k.kind)).join(', ')})], ${words})`);
        break;
    }
  }
  return out;
}
