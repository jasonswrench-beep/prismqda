//! The formula language and the transformations of the data editor must match R (specification 17.4).
//!
//! `reference/formula_reference.json` is made by `reference/make_formula_reference.R` with base R 4.5.3. Each formula is a text that
//! R evaluates itself (`eval(parse(text = formula), data)`) and PrismQDA must give the same answer for every case: the same numbers,
//! the same truth values, the same texts, and a missing value wherever R has one (R's own NaN and infinity are written as missing in
//! the file, and a value that PrismQDA cannot give as a finite number is missing too). The language is a subset of R's with R's
//! precedence and R's missing values; `round` is R's. The data are invented.

use prismqda_stats::formula::{evaluate_columns, parse, r_round, Value};
use prismqda_stats::transform::{cut, range_labels, rank_values, standardize};
use serde_json::Value as Json;

fn load() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/formula_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run make_formula_reference.R to create the reference file")).unwrap()
}

/// A column of numbers or texts, R's NA as None; R's JSON writer gives a single value without the list around it.
fn list(v: &Json) -> Vec<Json> {
    match v {
        Json::Array(a) => a.clone(),
        other => vec![other.clone()],
    }
}

fn numbers(v: &Json) -> Vec<Option<f64>> {
    list(v).iter().map(Json::as_f64).collect()
}

fn columns() -> (Vec<String>, Vec<Vec<Value>>) {
    let d = &load()["data"];
    let num = |key: &str| -> Vec<Value> { numbers(&d[key]).into_iter().map(|v| v.map_or(Value::Na, Value::Num)).collect() };
    let text: Vec<Value> = list(&d["g"]).iter().map(|v| v.as_str().map_or(Value::Na, |s| Value::Text(s.to_string()))).collect();
    (["a", "b", "c", "g", "Stat age"].map(String::from).to_vec(), vec![num("a"), num("b"), num("c"), text, num("stat_age")])
}

fn same(case: &str, row: usize, got: &Value, want: &Json, kind: &str) {
    match want {
        Json::Null => assert!(matches!(got, Value::Na) || matches!(got, Value::Num(x) if !x.is_finite()), "{case}, case {row}: R has a missing value but PrismQDA gives {got:?}"),
        Json::String(s) => assert_eq!(got, &Value::Text(s.clone()), "{case}, case {row}"),
        other => {
            let w = other.as_f64().unwrap();
            let g = match got {
                Value::Num(x) => *x,
                Value::Bool(b) => f64::from(u8::from(*b)),
                v => panic!("{case}, case {row}: R says {w} but PrismQDA gives {v:?}"),
            };
            assert!((g - w).abs() <= 1e-12 * w.abs().max(1.0), "{case}, case {row}: PrismQDA {g:e}, R {w:e}");
            if kind == "logical" {
                assert!(matches!(got, Value::Bool(_)), "{case}, case {row}: R gives a truth value but PrismQDA gives {got:?}");
            }
        }
    }
}

#[test]
fn every_formula_gives_the_answer_r_gives() {
    let (names, cols) = columns();
    let n = cols[0].len();
    let reference = load();
    let formulas = reference["formulas"].as_array().unwrap();
    assert!(formulas.len() >= 75, "dozens of formulas are compared: {}", formulas.len());
    let mut compared = 0;
    for f in formulas {
        let text = f["formula"].as_str().unwrap();
        let got = evaluate_columns(&parse(text).unwrap_or_else(|e| panic!("{text}: {e:?}")), &names, &cols).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        assert_eq!(got.len(), n, "{text}");
        let want = list(&f["values"]);
        for (i, g) in got.iter().enumerate() {
            // A formula of constants has one value in R, which is every case's value.
            same(text, i, g, &want[if want.len() == 1 { 0 } else { i }], f["type"].as_str().unwrap());
            compared += 1;
        }
    }
    assert!(compared > 1800, "compared {compared} values");
}

#[test]
fn round_is_rs_round_on_the_halves_where_binary_makes_them_fall_either_side() {
    for r in load()["rounding"].as_array().unwrap() {
        let digits = r["digits"].as_u64().unwrap() as u32;
        let (xs, want) = (numbers(&r["x"]), numbers(&r["expected"]));
        for (x, w) in xs.iter().zip(&want) {
            let (x, w) = (x.unwrap(), w.unwrap());
            let g = r_round(x, digits);
            assert!((g - w).abs() <= 1e-12 * w.abs().max(1.0), "round({x}, {digits}) is {g} but R says {w}");
        }
    }
}

#[test]
fn standardizing_ranking_and_cutting_match_scale_rank_and_cut() {
    let t = &load()["transforms"];
    let x = numbers(&t["x"]);
    let z = standardize(&x).unwrap();
    for (g, w) in z.iter().zip(numbers(&t["standardize"])) {
        match (g, w) {
            (Some(g), Some(w)) => assert!((g - w).abs() < 1e-12, "z {g} against {w}"),
            (None, None) => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(rank_values(&x, false), numbers(&t["rank_up"]));
    assert_eq!(rank_values(&x, true), numbers(&t["rank_down"]), "the highest first");
    let cuts: Vec<f64> = numbers(&t["cuts"]).into_iter().flatten().collect();
    let codes = |v: &Json| list(v).iter().map(|c| c.as_u64().map(|n| n as usize - 1)).collect::<Vec<_>>();
    assert_eq!(cut(&x, &cuts, true).unwrap(), codes(&t["cut_right"]));
    assert_eq!(cut(&x, &cuts, false).unwrap(), codes(&t["cut_left"]));
    assert_eq!(range_labels(&cuts, true), ["[0, 3]", "(3, 6]", "(6, 11]"]);
    assert_eq!(range_labels(&cuts, false), ["[0, 3)", "[3, 6)", "[6, 11]"]);
}

#[test]
fn what_cannot_be_standardized_or_cut_says_so() {
    assert!(standardize(&[Some(1.0)]).is_none(), "one value has no spread");
    assert!(standardize(&[Some(2.0), Some(2.0), None]).is_none(), "values that are all the same have no spread");
    assert!(cut(&[Some(1.0)], &[1.0], true).is_err());
    assert!(cut(&[Some(1.0)], &[3.0, 2.0], true).is_err(), "cut points must rise");
    assert_eq!(cut(&[Some(-1.0), Some(12.0), Some(f64::NAN)], &[0.0, 5.0, 10.0], true).unwrap(), vec![None, None, None], "outside every range is missing");
}

fn message(text: &str) -> String {
    match parse(text) {
        Err(e) => e.message,
        Ok(f) => {
            let names = vec!["a".to_string(), "g".to_string()];
            let cols = vec![vec![Value::Num(1.0)], vec![Value::Text("x".to_string())]];
            evaluate_columns(&f, &names, &cols).map(|_| "no error".to_string()).unwrap_or_else(|e| e.message)
        }
    }
}

#[test]
fn a_formula_that_cannot_be_read_or_worked_out_says_what_is_wrong_and_where() {
    assert!(message("a +").contains("ends where a value is expected"));
    assert!(message("a = 3").contains("write == to compare"));
    assert!(message("a b").contains("something is left over"));
    assert!(message("(a + 1").contains("not closed"));
    assert!(message("a + 1)").contains("left over") || message("a + 1)").contains("no ("));
    assert!(message("a > 1 > 2").contains("two comparisons in a row"));
    assert!(message("`a").contains("backtick is not closed"));
    assert!(message("\"abc").contains("quotation mark is not closed"));
    assert!(message("").contains("empty"));
    assert!(message("foo(a)").contains("there is no function named foo"));
    assert!(message("abs(a, 2)").contains("abs takes 1 value, and was given 2"));
    assert!(message("round(a, -1)").contains("whole number of digits"));
    assert!(message("zzz + 1").contains("there is no variable named zzz"));
    assert!(message("g + 1").contains("is text"));
    assert!(message("a # 2").contains("'#' is not something this language uses"));
    assert!(message("a %x% 2").contains("not an operator"));
    assert!(message("c(1, 2)").contains("only for listing"));
    assert_eq!(parse("a + ").unwrap_err().at, Some(4), "the position is where the formula ends");
    assert_eq!(parse("a + $").unwrap_err().at, Some(4), "or the sign at fault");
}

#[test]
fn the_variables_of_a_formula_are_listed_once_in_order() {
    assert_eq!(parse("b * a + b / a").unwrap().variables(), ["b", "a"]);
    assert_eq!(parse("`Stat age` + ifelse(g == \"A\", 1, 0)").unwrap().variables(), ["Stat age", "g"]);
    assert!(parse("1 + 2 * pi").unwrap().variables().is_empty());
}
