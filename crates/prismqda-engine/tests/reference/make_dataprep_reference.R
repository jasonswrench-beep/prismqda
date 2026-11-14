# Reference numbers for the data editor's date parts and group summaries, made with R 4.5.3 (base R only).
#   Rscript make_dataprep_reference.R
# writes dateparts.tsv (a date and each part of it as R gives it) and groupsummary.tsv (a value, its group, and what each summary of the
# group is as R's ave() and sd() give it).

dates <- c("2024-03-14", "2024-12-30", "2021-01-03", "2020-12-31", "2016-01-01", "2022-01-01", "2019-12-31", "2000-02-29",
           "1999-12-31", "2026-07-04", "2024-02-29", "1970-01-01", "2100-03-01", "2015-12-31", "2021-01-04", "2018-12-31",
           "2023-01-01", "2023-01-02", "2020-01-01", "2026-12-31", "1969-07-20", "2004-12-31", "2005-01-01", "2012-12-30")
d <- as.Date(dates)
parts <- data.frame(
  date = dates,
  year = as.integer(format(d, "%Y")),
  quarter = (as.integer(format(d, "%m")) - 1L) %/% 3L + 1L,
  month = as.integer(format(d, "%m")),
  week = as.integer(format(d, "%V")),
  day = as.integer(format(d, "%d")),
  doy = as.integer(format(d, "%j")),
  weekday = as.integer(format(d, "%u"))
)
write.table(parts, "dateparts.tsv", sep = "\t", quote = FALSE, row.names = FALSE)

x <- c(4.5, 7, 3, 9.25, 12, 1, 8, 6, 15, 2.5, 11, 5)
g <- c("a", "b", "a", "c", "b", "a", "c", "b", "a", "c", "b", "a")
fmt <- function(v) format(v, digits = 15)
out <- data.frame(
  x = fmt(x), g = g,
  mean = fmt(ave(x, g, FUN = mean)), sum = fmt(ave(x, g, FUN = sum)), median = fmt(ave(x, g, FUN = median)),
  min = fmt(ave(x, g, FUN = min)), max = fmt(ave(x, g, FUN = max)), sd = fmt(ave(x, g, FUN = sd)),
  n = ave(x, g, FUN = length)
)
write.table(out, "groupsummary.tsv", sep = "\t", quote = FALSE, row.names = FALSE)
