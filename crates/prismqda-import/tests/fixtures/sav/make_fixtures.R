# Builds the SPSS .sav test fixtures for prismqda-import and writes what R reads back from each one.
#
# The files are written by `haven::write_sav` (which uses the ReadStat library), so they come from a
# writer that has nothing to do with the Rust reader in src/sav.rs. Next to each file this script
# writes `<name>.expected.json`: the contents as read back by `haven::read_sav(user_na = TRUE)` (also
# ReadStat, a different code path from the Rust reader). tests/sav_fixtures.rs compares the Rust
# reader's output to that JSON cell by cell.
#
# All data here is made up. Nothing comes from a real study.
#
# Needs R 4.x and the CRAN package `haven` (install it into a user library; nothing system-wide):
#   install.packages("haven", lib = Sys.getenv("R_LIBS_USER"))
# Run it from anywhere; output goes into the folder that holds this script:
#   Rscript make_fixtures.R
# On the build machine for this project, R must be pinned to the efficiency cores:
#   start "" /b /wait /affinity FFFF0000 "C:\Program Files\R\R-4.5.3\bin\Rscript.exe" make_fixtures.R
#
# What the JSON holds for each column: name, variable label, kind (number, text, date, datetime),
# the SPSS print format, the measure haven writes for that kind of column, value labels (key as text),
# user-missing values, and every cell (null for system-missing or an empty string).
#
# Things the Rust reader is expected to do that R does not do by itself:
#  * Strings lose trailing spaces (done here with sub()).
#  * MOYR, QYR and WKYR columns come back from haven as plain seconds; here they are turned into
#    dates with R's own date code, so the expected text does not depend on the Rust conversion.
#  * Time-of-day (TIME) columns stay numbers of seconds.
#  * The measure is not readable from R, so it is written down from the class of the source column,
#    which is how haven chooses it (ordered factor: ordinal; factor or text: nominal; the rest: scale).
#    The raw display record of the files was also checked byte by byte outside of R.

args_all <- commandArgs(trailingOnly = FALSE)
file_arg <- sub("^--file=", "", args_all[grepl("^--file=", args_all)])
out_dir <- if (length(file_arg) == 1) dirname(normalizePath(file_arg, winslash = "/")) else getwd()
user_lib <- Sys.getenv("R_LIBS_USER")
if (nzchar(user_lib) && dir.exists(user_lib)) .libPaths(c(user_lib, .libPaths()))
suppressPackageStartupMessages(library(haven))

SPSS_EPOCH_TO_UNIX <- 12219379200  # seconds from 1582-10-14 to 1970-01-01

# ---- small JSON writer (no extra packages) -------------------------------------------------------

jstr <- function(s) {
  if (length(s) == 0 || is.na(s)) return("null")
  s <- enc2utf8(as.character(s))
  s <- gsub("\\", "\\\\", s, fixed = TRUE)
  s <- gsub("\"", "\\\"", s, fixed = TRUE)
  s <- gsub("\n", "\\n", s, fixed = TRUE)
  s <- gsub("\r", "\\r", s, fixed = TRUE)
  s <- gsub("\t", "\\t", s, fixed = TRUE)
  paste0("\"", s, "\"")
}
jnum <- function(v) if (is.na(v)) "null" else sprintf("%.17g", v)
jarr <- function(items) paste0("[", paste(items, collapse = ","), "]")
jobj <- function(keys, items) {
  if (length(keys) == 0) return("{}")
  paste0("{", paste0(vapply(keys, jstr, ""), ":", items, collapse = ","), "}")
}
num_text <- function(v) format(v, digits = 15, scientific = FALSE, trim = TRUE)

# ---- describing a column the way R reads it ------------------------------------------------------

measure_of <- function(src) {
  if (is.ordered(src)) "ordinal"
  else if (is.factor(src) || is.character(src)) "nominal"
  else "scale"
}

column_json <- function(x, name, src) {
  # attr() matches names by their start unless exact = TRUE, which would mix up "label" and "labels".
  fmt <- attr(x, "format.spss", exact = TRUE)
  fmt_base <- if (is.null(fmt)) "" else toupper(sub("[0-9.]*$", "", fmt))
  kind <- if (inherits(x, "POSIXct")) "datetime"
          else if (inherits(x, "Date")) "date"
          else if (fmt_base %in% c("MOYR", "QYR", "WKYR")) "date"
          else if (is.character(x)) "text"
          else "number"
  raw <- as.vector(unclass(x))
  cells <- if (kind == "datetime") {
    vapply(format(x, "%Y-%m-%dT%H:%M:%S", tz = "UTC"), jstr, "", USE.NAMES = FALSE)
  } else if (inherits(x, "Date")) {
    vapply(format(x, "%Y-%m-%d"), jstr, "", USE.NAMES = FALSE)
  } else if (kind == "date") {
    d <- as.POSIXct(raw - SPSS_EPOCH_TO_UNIX, origin = "1970-01-01", tz = "UTC")
    vapply(format(d, "%Y-%m-%d"), jstr, "", USE.NAMES = FALSE)
  } else if (kind == "text") {
    vapply(sub(" +$", "", raw), function(s) if (is.na(s) || s == "") "null" else jstr(s), "", USE.NAMES = FALSE)
  } else {
    vapply(as.numeric(raw), jnum, "", USE.NAMES = FALSE)
  }
  labels <- attr(x, "labels", exact = TRUE)
  vl <- if (is.null(labels)) "{}" else {
    keys <- if (is.character(labels)) sub(" +$", "", unname(labels)) else vapply(unname(labels), num_text, "")
    jobj(keys, vapply(names(labels), jstr, "", USE.NAMES = FALSE))
  }
  na_values <- attr(x, "na_values", exact = TRUE)
  na_range <- attr(x, "na_range", exact = TRUE)
  nav <- if (is.null(na_values)) "[]" else {
    jarr(vapply(na_values, function(v) if (is.character(v)) jstr(sub(" +$", "", v)) else jstr(num_text(v)), "", USE.NAMES = FALSE))
  }
  nar <- if (is.null(na_range)) "null" else {
    jarr(vapply(na_range, function(v) jstr(if (is.infinite(v)) (if (v < 0) "LOWEST" else "HIGHEST") else num_text(v)), "", USE.NAMES = FALSE))
  }
  lab <- attr(x, "label", exact = TRUE)
  measure <- if (kind %in% c("date", "datetime")) "date" else measure_of(src)
  paste0(
    "{\"name\":", jstr(name),
    ",\"label\":", if (is.null(lab)) "null" else jstr(lab),
    ",\"kind\":", jstr(kind),
    ",\"format_spss\":", if (is.null(fmt)) "null" else jstr(fmt),
    ",\"measure\":", jstr(measure),
    ",\"value_labels\":", vl,
    ",\"na_values\":", nav,
    ",\"na_range\":", nar,
    ",\"values\":", jarr(cells), "}"
  )
}

write_text <- function(path, text) writeBin(charToRaw(enc2utf8(text)), path)

# Writes one fixture and the JSON of what R reads back from it.
make_fixture <- function(df, file, compress) {
  path <- file.path(out_dir, file)
  write_sav(df, path, compress = compress)
  back <- read_sav(path, user_na = TRUE)
  stopifnot(nrow(back) == nrow(df), ncol(back) == ncol(df))
  cols <- vapply(seq_along(back), function(i) column_json(back[[i]], names(back)[i], df[[i]]), "")
  json <- paste0(
    "{\"file\":", jstr(file),
    ",\"written_by\":", jstr(paste0("haven ", as.character(packageVersion("haven")), " write_sav(compress = \"", compress, "\")")),
    ",\"compression\":", jstr(compress),
    ",\"nrow\":", nrow(back),
    ",\"columns\":", jarr(cols), "}\n"
  )
  write_text(file.path(out_dir, sub("\\.[A-Za-z0-9]+$", ".expected.json", file)), json)
  size <- file.info(path)$size
  cat(sprintf("%-28s %6d bytes  %d rows  %d columns\n", file, size, nrow(back), ncol(back)))
  if (size > 50 * 1024) stop("fixture is over 50 KB: ", file)
}

lab <- function(x, label) { attr(x, "label") <- label; x }

# A string of n letters and digits with a space inside every ninth place, never first or last.
fake_text <- function(n, seed) {
  set.seed(seed)
  ch <- sample(c(letters, LETTERS, 0:9), n, replace = TRUE)
  i <- seq_len(n)
  ch[i %% 9 == 0 & i > 1 & i < n] <- " "
  paste(ch, collapse = "")
}

# ---- basic: labels, measures, strings, doubles that are hard to round-trip -----------------------

basic <- tibble::tibble(
  id = lab(1:8, "Respondent number"),
  gender = labelled(c(1, 2, 3, 2, 1, 2, NA, 1),
                    labels = c(Female = 1, Male = 2, `Non-binary` = 3), label = "Gender identity (self-described)"),
  satisf = lab(factor(c("High", "Low", NA, "Very high", "Medium", "Very low", "High", "Low"),
                      levels = c("Very low", "Low", "Medium", "High", "Very high"), ordered = TRUE),
               "Satisfaction with the programme"),
  region = lab(factor(c("North", "South", "East", "South", "West", "North", "East", "West")), "Region"),
  age = lab(c(34, 51, NA, 27.5, 62, 45, 19, 38), "Age in years"),
  edges = lab(c(-100, -99, 0, 100, 151, 152, 1.5, NA), "Values at the edges of the byte-compression range"),
  awkward = lab(c(0.1 + 0.2, 1 / 3, 123456789.123456789, 1e-300, 1e300, -1e-300, -12.5, 2^53), "Doubles that are hard to round-trip"),
  half = labelled(c(1, 2.5, 2.5, 1, NA, 2.5, 1, 2),
                  labels = c(One = 1, `Two and a half` = 2.5, Two = 2), label = "Label key that is not a whole number"),
  str1 = c("a", "b", "c", "d", "e", "f", "g", "h"),
  str8 = c("12345678", "abc", "", "x y z", "ABCDEFGH", NA, "9", "tail"),
  str9 = c("123456789", "short", NA, "", "nine char", "two words", "x", "a b c d e"),
  str16 = c("sixteen chars!!!", "short", "", NA, "mid length text", "x", "0123456789abcde", "tab\there"),
  comment = c("I felt welcome from the first day and kept coming back.",
              "The cost was the hard part for me and my family overall.",
              "",
              "Short answer",
              "   leading spaces kept, trailing removed   ",
              "Quotes \"inside\", a backslash \\ and more words to follow here",
              "Line one\nline two of the same answer given here",
              NA),
  fruit = labelled(c("a", "b", "c", "a", NA, "b", "c", "a"),
                   labels = c(Apple = "a", Banana = "b", Cherry = "c"), label = "Fruit code (short text)"),
  fruit_long = labelled(c("alphaalphaalpha", "beta", "gamma", "beta", "alphaalphaalpha", "gamma", NA, "beta"),
                        labels = c(A = "alphaalphaalpha", B = "beta"), label = "Fruit code (longer than 8 bytes)")
)
make_fixture(basic, "basic_none.sav", "none")
make_fixture(basic, "basic_byte.sav", "byte")
make_fixture(basic, "basic_zsav.zsav", "zsav")

# ---- missing values -------------------------------------------------------------------------------

missing <- tibble::tibble(
  m_one = labelled_spss(c(1, 2, 99, NA, 3, 99, 1, 2, 3, NA), labels = c(Yes = 1, No = 2, Unknown = 99), na_values = 99, label = "One missing value"),
  m_three = labelled_spss(c(1, 7, 8, 9, 2, 3, 7, 1, 9, 2), labels = c(Low = 1, High = 3, `Not asked` = 7, Refused = 8, `Not sure` = 9),
                          na_values = c(7, 8, 9), label = "Three missing values"),
  m_range = labelled_spss(c(1, 95, 97, 3, 90, 99, 2, 100, 89, NA), na_range = c(90, 99), label = "Missing range"),
  m_range_one = labelled_spss(c(1, 95, 0, 3, 99, 0, 2, 100, 89, NA), na_range = c(90, 99), na_values = 0, label = "Missing range plus one value"),
  m_low_range = labelled_spss(c(1, -5, 3, 4, 0, -1, 2, 5, 6, NA), na_range = c(-Inf, 0), label = "Missing range from lowest"),
  m_high_range = labelled_spss(c(1, 500, 3, 4, 99, 100, 2, 5, 6, NA), na_range = c(99, Inf), label = "Missing range to highest"),
  m_neg = labelled_spss(c(1, -99, -98, 4, 2, 3, -99, 1, 2, NA), na_values = c(-99, -98), label = "Negative missing codes"),
  m_dec = labelled_spss(c(1, 99.5, 3, 4, 99.5, 1, 2, 3, 4, NA), na_values = 99.5, label = "Missing code with a decimal"),
  m_str = labelled_spss(c("a", "b", "N/A", "b", "a", "N/A", "b", "a", "b", ""), na_values = "N/A", label = "Text with a missing code"),
  m_str3 = labelled_spss(c("a", "bb", "N/A", "zzzzzzzzzzzz", "a", "bb", "N/A", "a", "b", ""), na_values = c("N/A", "bb", "zzzzzzzzzzzz"),
                         label = "Longer text with three missing codes"),
  sysmis_only = lab(c(NA, 1, NA, 2, NA, 3, NA, 4, NA, 5), "System-missing in alternate rows"),
  all_missing = lab(rep(NA_real_, 10), "Nothing but system-missing")
)
make_fixture(missing, "missing_byte.sav", "byte")

# ---- dates, date-times and times -------------------------------------------------------------------

dates_of <- function(fmt) {
  x <- as.Date(c("2024-03-05", NA, "1999-12-31", "2000-01-01", "1970-01-01", "1900-03-01", "2024-02-29", "1582-10-15", "2038-01-19", "9999-12-31"))
  attr(x, "format.spss") <- fmt
  lab(x, paste("Date shown as", fmt))
}
secs_as <- function(fmt) {
  # A date counted in SPSS seconds but shown in a format haven does not treat as a date.
  x <- (as.numeric(as.Date(c("2024-03-05", NA, "1999-12-31", "2000-01-01", "1970-01-01", "1900-03-01", "2024-02-29", "1582-10-15", "2038-01-19", "9999-12-31"))) * 86400) + SPSS_EPOCH_TO_UNIX
  attr(x, "format.spss") <- fmt
  lab(x, paste("Seconds shown as", fmt))
}
dates <- tibble::tibble(
  d_date = dates_of("DATE11"),
  d_adate = dates_of("ADATE10"),
  d_edate = dates_of("EDATE10"),
  d_sdate = dates_of("SDATE10"),
  d_jdate = dates_of("JDATE7"),
  d_moyr = secs_as("MOYR6"),
  d_qyr = secs_as("QYR6"),
  d_wkyr = secs_as("WKYR8"),
  d_month = secs_as("MONTH8"),
  d_wkday = secs_as("WKDAY8"),
  tm = lab(hms::as_hms(c(3661, NA, 0, 86399, 45296, 1, 59, 3600, 7200, 12)), "Time of day"),
  dt = lab(as.POSIXct(c("2024-03-05 13:45:10", NA, "1999-12-31 23:59:59", "2000-01-01 00:00:00", "1969-07-20 20:17:40",
                        "2024-02-29 12:00:00", "1900-03-01 00:00:01", "2038-01-19 03:14:08", "2024-12-31 23:59:59", "9999-12-31 23:59:59"), tz = "UTC"),
           "Date and time")
)
make_fixture(dates, "dates_none.sav", "none")

# ---- non-ASCII text ---------------------------------------------------------------------------------

straddle <- paste0("x", strrep("\u65e5", 40), "end")  # multi-byte characters across an 8-byte boundary
unicode <- tibble::tibble(
  latin = lab(c("Caf\u00e9 Zo\u00eb", "ni\u00f1o \u00fcber \u00df", "\u00c5ngstr\u00f6m", "", NA, "na\u00efve fa\u00e7ade"), "Accents"),
  cjk = lab(c("\u65e5\u672c\u8a9e\u306e\u30c6\u30ad\u30b9\u30c8", "\u4e2d\u6587\u6587\u672c", "\ud55c\uad6d\uc5b4", "", "\u65e5", NA), "CJK text"),
  emoji = lab(c("\U0001F600 smile", "\U0001F44D\U0001F3FD thumbs up", "plain", "", "\U0001F9D1\u200D\U0001F4BB", NA), "Emoji"),
  cyrillic_rtl = lab(c("\u041f\u0440\u0438\u0432\u0435\u0442 \u043c\u0438\u0440", "\u05e9\u05dc\u05d5\u05dd \u05e2\u05d5\u05dc\u05dd", "\u0645\u0631\u062d\u0628\u0627 \u0628\u0627\u0644\u0639\u0627\u0644\u0645", "", NA, "mixed \u0645\u0631\u062d\u0628\u0627 text"), "Other scripts"),
  combining = lab(c("e\u0301 (decomposed)", "\u00e9 (precomposed)", "n\u0303", "", NA, "a\u030a"), "Combining marks left as the file has them"),
  straddle = lab(c(straddle, "short", "", NA, "\u65e5\u65e5\u65e5", "x"), "Characters across slot boundaries"),
  name_fr = labelled(c(1, 2, 1, NA, 2, 3),
                     labels = stats::setNames(c(1, 2, 3), c("Premi\u00e8re", "Deuxi\u00e8me", "\u7b2c\u4e09")),
                     label = "Ann\u00e9e d'\u00e9tude"),
  name_jp = lab(c(1, 2, 3, 4, 5, 6), "\u540d\u524d\u306e\u30e9\u30d9\u30eb"),
  name_long = lab(c(1, 2, 3, 4, 5, 6), "Long name"),
  name_case = lab(c(1, 2, 3, 4, 5, 6), "Case is kept"),
  uni_label = labelled(c("x", "y", "x", "y", NA, "x"),
                       labels = stats::setNames(c("x", "y"), c("Ja \u2713", "Nein \u2717")),
                       label = "Unicode value labels")
)
# Names with accents, CJK, a very long name (over 8 bytes), and mixed case. Backticks cannot hold \u escapes.
names(unicode)[names(unicode) == "name_fr"] <- "ann\u00e9e"
names(unicode)[names(unicode) == "name_jp"] <- "\u540d\u524d"
names(unicode)[names(unicode) == "name_long"] <- "a_really_long_variable_name_that_is_far_longer_than_eight_bytes"
names(unicode)[names(unicode) == "name_case"] <- "Mixed_Case_Name"
make_fixture(unicode, "unicode_byte.sav", "byte")

# ---- long strings (over 255 bytes use several segments) -----------------------------------------------

long_n <- c(9, 16, 100, 255, 256, 300, 504, 505, 1000)
long_cols <- lapply(long_n, function(n) {
  v <- c(fake_text(n, n), fake_text(max(1, n %/% 3), n + 1), "", NA, fake_text(n - 1, n + 2))
  lab(v, paste("Text up to", n, "bytes"))
})
names(long_cols) <- paste0("s", long_n)
# A space sits exactly on the 252-byte segment boundary, and a character straddles it.
long_cols$s300[2] <- paste0(strrep("a", 251), " ", strrep("b", 40))
long_cols$s300[5] <- paste0(strrep("c", 252), strrep("d", 10), " e")
long_cols$s505[5] <- paste0(strrep("1", 504), "2")
straddle_long <- paste0("x", strrep("\u65e5", 120))  # 361 bytes; the 252nd byte is inside a character
long_cols$straddle361 <- lab(c(straddle_long, "short", "", NA, paste0("y", strrep("\u00e9", 100))), "Multi-byte characters across a segment boundary")
# Numbers before and after the long texts: their measure, label and value labels must line up with the
# right variable although a long text takes several variable records.
long_cols <- c(
  list(head_num = lab(c(1.5, 2, NA, 4, 5), "Number before the long texts")),
  long_cols,
  list(
    tail_ordered = lab(factor(c("lo", "mid", "hi", NA, "lo"), levels = c("lo", "mid", "hi"), ordered = TRUE), "Ordered after the long texts"),
    tail_labelled = labelled(c(1, 2, 2, NA, 1), labels = c(Yes = 1, No = 2), label = "Labelled after the long texts"),
    tail_text = lab(c("a", "b", "c", "d", "e"), "Short text after the long texts")
  )
)
long_df <- tibble::as_tibble(long_cols)
make_fixture(long_df, "longstrings_none.sav", "none")
make_fixture(long_df, "longstrings_byte.sav", "byte")
make_fixture(long_df, "longstrings_zsav.zsav", "zsav")

# ---- a wide file (130 variables; labels point at columns past the 100th) ---------------------------------

set.seed(42)
wide_rows <- 20
wide_cols <- list()
for (i in 1:130) {
  nm <- sprintf("v%03d", i)
  lbl <- sprintf("Variable %d", i)
  if (i %% 5 == 0) {
    x <- lab(sprintf("text %d row %d%s", i, 1:wide_rows, ifelse(1:wide_rows %% 4 == 0, " with more words", "")), lbl)
  } else if (i %% 7 == 0) {
    x <- labelled(sample(c(1, 2, 3, NA), wide_rows, replace = TRUE), labels = c(One = 1, Two = 2, Three = 3), label = lbl)
  } else if (i %% 11 == 0) {
    x <- labelled_spss(sample(c(1, 2, 9, NA), wide_rows, replace = TRUE), labels = c(Yes = 1, No = 2, `Don't know` = 9), na_values = 9, label = lbl)
  } else {
    x <- lab(round(runif(wide_rows, -50, 150)) + (if (i %% 13 == 0) 0.5 else 0), lbl)
  }
  wide_cols[[nm]] <- x
}
wide <- tibble::as_tibble(wide_cols)
make_fixture(wide, "wide_byte.sav", "byte")

# ---- no rows ---------------------------------------------------------------------------------------------

empty_src <- tibble::tibble(
  id = lab(1:3, "Id"),
  grp = labelled(c(1, 2, 1), labels = c(A = 1, B = 2), label = "Group"),
  note = lab(c("a", "b", "c"), "A note"),
  seen = lab(as.Date(c("2024-01-01", "2024-01-02", "2024-01-03")), "Seen on")
)
make_fixture(empty_src[0, ], "empty_none.sav", "none")
make_fixture(empty_src[0, ], "empty_byte.sav", "byte")

# ---- a small survey for the end-to-end import test --------------------------------------------------------

survey <- tibble::tibble(
  respondent_id = lab(1001:1012, "Respondent ID"),
  group = labelled(c(1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2), labels = c(Treatment = 1, Control = 2), label = "Study group"),
  age = labelled_spss(c(34, 51, -99, 27, 62, 45, 19, 38, 44, 29, 57, -99), na_values = -99, label = "Age in years"),
  satisfaction = lab(factor(c("High", "Low", "Medium", "High", "Very high", "Low", "Medium", "High", "Low", "Medium", "High", "Very high"),
                            levels = c("Very low", "Low", "Medium", "High", "Very high"), ordered = TRUE), "Satisfaction"),
  why = lab(c("I felt welcome from the first day and kept coming back.",
              "The cost was the hard part for me and my family overall.",
              "Nobody explained what to expect, so I stayed away for a while.",
              "My friends came with me and that made all the difference to me.",
              "I learned more here than in any class I have taken before now.",
              "The staff listened to what I said and acted on it right away.",
              "Parking was difficult but the sessions themselves were worth it.",
              "I would have liked more time to talk with the other participants.",
              "It felt rushed at the start but settled down after the first week.",
              "Child care made it possible for me to attend each of the meetings.",
              "I was nervous about joining, yet everyone made me feel at ease quickly.",
              "The materials were clear and I shared them with my neighbours afterwards."), "Why did you join?"),
  advice = lab(c("Bring a friend.", "", "Be patient with the process.", "Ask questions early.", NA, "Start sooner.",
                 "Come with an open mind.", "", "Take notes.", "Stay to the end.", NA, "Say yes."), "Advice for newcomers")
)
make_fixture(survey, "survey_byte.sav", "byte")

cat("done\n")
