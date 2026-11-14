# Reference values for the formula language and the transformations of the data editor, computed by base R.
#
# Run with:  Rscript make_formula_reference.R
# It writes formula_reference.json next to this script. tests/formula_reference.rs requires PrismQDA to give the same answer for the
# same text, case by case.
#
#   formulas        the text is evaluated by R itself: eval(parse(text = formula), data). The language is a subset of R's, with R's
#                   precedence, R's missing values, and R's round. The functions that work across variables of one case have no
#                   one-word R equivalent, so their expected values are written out (rowMeans and friends)
#   standardize     scale(x)
#   rank            rank(x, na.last = "keep"), and rank(-x) for the highest first
#   cut             as.integer(cut(x, breaks, right, include.lowest = TRUE))
#
# The data are synthetic and invented here. Nothing comes from a real study.

suppressMessages(library(jsonlite))

script_dir <- function() {
  args <- commandArgs(trailingOnly = FALSE)
  hit <- grep("^--file=", args, value = TRUE)
  if (length(hit)) dirname(normalizePath(sub("^--file=", "", hit[1]))) else getwd()
}
num <- function(v) as.numeric(unname(v))

df <- data.frame(
  a = c(3, 5, NA, 2, 8, 0, -1, 4.5, 7, NA, 1, 6, 2.675, 0.5, 1.5, 2.5, 10, 12, 0.15, 9.99, 4, 5, 6, 7),
  b = c(2, NA, 4, 1, 3, 5, 0, 2.5, 6, 3, NA, 8, 1.25, 2, 0.35, 9, 3, NA, 7, 0, 5, 1, 2, 3),
  c = c(-6, -3, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, -1, -2, 11, 12, -9, 13, 14, 15, 16, -4, -5),
  g = c("A", "B", "C", NA, "A", "B", "C", "A", "B", NA, "A", "A", "B", "C", "A", "B", "C", "A", NA, "B", "C", "A", "B", "C"),
  stringsAsFactors = FALSE
)
df[["Stat age"]] <- c(21, 25, 33, 41, NA, 38, 44, 52, 31, 36, 47, 55, 29, 30, 31, 32, 33, 34, 35, NA, 37, 39, 40, 42)

formulas <- c(
  "a + b", "a - b * 2", "a / b", "a ^ 2", "-a ^ 2", "2 ^ 3 ^ 2", "(a + b) * c", "a %% 3", "a %/% 3", "c %% 4", "c %/% 4", "-7 %% 3",
  "a * 2 + b / 4 - c", "a ^ -1", "a * b %% 2", "-c", "+a", "1e3 * a", ".5 * a",
  "a > b", "a == b", "a != 3", "a >= 2 & b < 5", "a > 1 | is.na(b)", "!(a > 2)", "!a > 2", "!is.na(a) & a < 5",
  "g == \"A\"", "g != \"A\"", "g %in% c(\"A\", \"B\")", "a %in% c(1, 2, NA)", "g == 'B'", "a > 2 & NA", "a > 2 | NA", "FALSE & NA", "TRUE | NA", "NA + 1", "NA ^ 0", "1 ^ NA",
  "abs(c)", "sqrt(abs(c))", "exp(a / 10)", "log(b)", "log(b, 2)", "log2(b)", "log10(b + 1)", "floor(c / 3)", "ceiling(c / 3)", "trunc(c / 3)",
  "round(a)", "round(a, 1)", "round(a, 2)", "round(c / 7, 3)", "round(b / 8, 1)", "round(a * 100)",
  "pmin(a, b)", "pmax(a, b, c)", "pmin(a, 4)", "ifelse(a > 2, a, b)", "ifelse(g == \"A\", 1, 0)", "ifelse(is.na(a), -99, a)", "ifelse(a > b, \"high\", \"low\")",
  "is.na(a)", "is.na(g)", "as.numeric(g == \"A\")", "as.numeric(a > 2)", "`Stat age` * 2 + 1", "`Stat age` - mean_placeholder",
  "sin(a) + cos(b)", "tan(c / 10)", "pi * a", "(a + b) / (c + 100)", "a * (b + c) ^ 2"
)
formulas <- formulas[!grepl("mean_placeholder", formulas)]

kind <- function(v) if (is.logical(v)) "logical" else if (is.numeric(v)) "number" else "text"
eval_one <- function(f) {
  v <- eval(parse(text = f), df)
  list(formula = f, type = kind(v), values = if (is.numeric(v) || is.logical(v)) as.numeric(v) else v)
}
results <- lapply(formulas, eval_one)

# Across the variables of one case.
m <- cbind(df$a, df$b, df$c)
all_na <- function(f) apply(m, 1, function(r) all(is.na(r)))
across <- list(
  list(formula = "rowmean(a, b, c)", type = "number", values = ifelse(all_na(), NA, rowMeans(m, na.rm = TRUE))),
  list(formula = "rowsum(a, b, c)", type = "number", values = ifelse(all_na(), NA, rowSums(m, na.rm = TRUE))),
  list(formula = "rowmin(a, b, c)", type = "number", values = apply(m, 1, function(r) if (all(is.na(r))) NA_real_ else min(r, na.rm = TRUE))),
  list(formula = "rowmax(a, b)", type = "number", values = apply(m[, 1:2], 1, function(r) if (all(is.na(r))) NA_real_ else max(r, na.rm = TRUE))),
  list(formula = "nmiss(a, b, g)", type = "number", values = rowSums(is.na(cbind(df$a, df$b, df$g))))
)
results <- c(results, lapply(across, function(r) { r$values <- num(r$values); r }))

# round() on the numbers where binary representation makes a half fall on either side, checked on its own.
halves <- c(0.5, 1.5, 2.5, 3.5, -0.5, -1.5, -2.5, 2.675, 1.005, 0.15, 0.25, 0.35, 0.45, 2.345, 10.555, 1234.5678, 0.1 + 0.2, 5.5, 6.5)
rounding <- lapply(c(0, 1, 2, 3), function(d) list(digits = d, x = halves, expected = num(round(halves, d))))

x <- c(4, NA, 9, 2, 2, 7, NA, 11, 5, 5, 5, 0)
transforms <- list(
  x = x,
  standardize = num(as.numeric(scale(x))),
  rank_up = num(rank(x, na.last = "keep")),
  rank_down = num(rank(-x, na.last = "keep")),
  cuts = c(0, 3, 6, 11),
  cut_right = as.integer(cut(x, breaks = c(0, 3, 6, 11), right = TRUE, include.lowest = TRUE)),
  cut_left = as.integer(cut(x, breaks = c(0, 3, 6, 11), right = FALSE, include.lowest = TRUE))
)

result <- list(
  data = list(a = df$a, b = df$b, c = df$c, g = df$g, stat_age = df[["Stat age"]]),
  formulas = results, rounding = rounding, transforms = transforms
)
path <- file.path(script_dir(), "formula_reference.json")
write(toJSON(result, digits = NA, auto_unbox = TRUE, na = "null", pretty = TRUE), path)
cat("wrote", path, "\n")
cat("R", R.version.string, "\n")
