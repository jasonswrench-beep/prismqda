//! The formula language of the data editor: the arithmetic, comparisons, and functions a person writes to compute a new variable
//! from others, such as `(score - mean_score) / 10`, `ifelse(age >= 18 & site == "North", 1, 0)`, or `log(income + 1)`.
//!
//! The syntax is a subset of R's, with R's precedence, R's treatment of missing values (a missing operand makes the result missing,
//! except that `FALSE & NA` is `FALSE` and `TRUE | NA` is `TRUE`), and R's `round`, so that the same text can be given to R and
//! must give the same answer; `tests/formula_reference.rs` does that. A name with spaces or other odd characters is written in
//! backticks, as in R: `` `Stat age` * 2 ``. Functions that work across several variables of one case (R's `rowMeans` and
//! its kin) are `rowmean`, `rowsum`, `rowmin`, `rowmax`, and `nmiss`.

use std::collections::BTreeSet;

/// What a formula works on and gives: a number, a truth value, a text, or nothing (missing).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Na,
    Num(f64),
    Bool(bool),
    Text(String),
}

/// A formula that could not be read or worked out, in words, with where in the text it went wrong when that is known.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FormulaError {
    pub message: String,
    /// The position (in characters from 0) of the word or sign at fault.
    pub at: Option<usize>,
}

fn err<T>(message: impl Into<String>, at: Option<usize>) -> Result<T, FormulaError> {
    Err(FormulaError { message: message.into(), at })
}

// ---------------------------------------------------------------- reading

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Name(String),
    Op(&'static str),
    Special(String),
    Open,
    Close,
    Comma,
}

fn tokens(text: &str) -> Result<Vec<(Tok, usize)>, FormulaError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at = i;
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let s: String = chars[start..i].iter().collect();
            match s.parse::<f64>() {
                Ok(v) => out.push((Tok::Num(v), at)),
                Err(_) => return err(format!("'{s}' is not a number"), Some(at)),
            }
        } else if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return err("a quotation mark is not closed", Some(at)),
                    Some('\\') if i + 1 < chars.len() => {
                        s.push(chars[i + 1]);
                        i += 2;
                    }
                    Some(&q) if q == quote => {
                        i += 1;
                        break;
                    }
                    Some(&other) => {
                        s.push(other);
                        i += 1;
                    }
                }
            }
            out.push((Tok::Str(s), at));
        } else if c == '`' {
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return err("a backtick is not closed", Some(at)),
                    Some('`') => {
                        i += 1;
                        break;
                    }
                    Some(&other) => {
                        s.push(other);
                        i += 1;
                    }
                }
            }
            if s.is_empty() {
                return err("a name in backticks is empty", Some(at));
            }
            out.push((Tok::Name(s), at));
        } else if c.is_alphabetic() || c == '.' || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            out.push((Tok::Name(chars[start..i].iter().collect()), at));
        } else if c == '%' {
            let end = chars[i + 1..].iter().position(|&x| x == '%').map(|p| i + 1 + p);
            let Some(end) = end else { return err("a % sign is not closed", Some(at)) };
            let s: String = chars[i..=end].iter().collect();
            if !matches!(s.as_str(), "%in%" | "%%" | "%/%") {
                return err(format!("'{s}' is not an operator this language has (it has %in%, %%, and %/%)"), Some(at));
            }
            i = end + 1;
            out.push((Tok::Special(s), at));
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let op: Option<&'static str> = match two.as_str() {
                "==" => Some("=="),
                "!=" => Some("!="),
                "<=" => Some("<="),
                ">=" => Some(">="),
                "&&" => Some("&"),
                "||" => Some("|"),
                _ => None,
            };
            if let Some(op) = op {
                out.push((Tok::Op(op), at));
                i += 2;
                continue;
            }
            let one: Option<Tok> = match c {
                '+' => Some(Tok::Op("+")),
                '-' => Some(Tok::Op("-")),
                '*' => Some(Tok::Op("*")),
                '/' => Some(Tok::Op("/")),
                '^' => Some(Tok::Op("^")),
                '<' => Some(Tok::Op("<")),
                '>' => Some(Tok::Op(">")),
                '&' => Some(Tok::Op("&")),
                '|' => Some(Tok::Op("|")),
                '!' => Some(Tok::Op("!")),
                '(' => Some(Tok::Open),
                ')' => Some(Tok::Close),
                ',' => Some(Tok::Comma),
                _ => None,
            };
            match one {
                Some(t) => {
                    out.push((t, at));
                    i += 1;
                }
                None if c == '=' => return err("a single = is not a comparison; write == to compare", Some(at)),
                None => return err(format!("'{c}' is not something this language uses"), Some(at)),
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Na,
    Var(String, usize),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Special(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>, usize),
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }
    fn here(&self) -> usize {
        self.toks.get(self.pos).map_or(self.end, |t| t.1)
    }
    fn take(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.0.clone());
        self.pos += 1;
        t
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == op)
    }

    // R's precedence, lowest first: | and &, then !, then comparison, then + -, then * /, then %in% %% %/%, then unary + -, then ^.
    fn or(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.and()?;
        while self.is_op("|") {
            self.pos += 1;
            let right = self.and()?;
            left = Expr::Binary("|", Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.not()?;
        while self.is_op("&") {
            self.pos += 1;
            let right = self.not()?;
            left = Expr::Binary("&", Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn not(&mut self) -> Result<Expr, FormulaError> {
        if self.is_op("!") {
            self.pos += 1;
            let inner = self.not()?;
            return Ok(Expr::Unary("!", Box::new(inner)));
        }
        self.comparison()
    }
    fn comparison(&mut self) -> Result<Expr, FormulaError> {
        let left = self.additive()?;
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            if self.is_op(op) {
                self.pos += 1;
                let right = self.additive()?;
                if matches!(self.peek(), Some(Tok::Op(o)) if ["==", "!=", "<=", ">=", "<", ">"].contains(o)) {
                    return err("two comparisons in a row; join them with & or |", Some(self.here()));
                }
                return Ok(Expr::Binary(op, Box::new(left), Box::new(right)));
            }
        }
        Ok(left)
    }
    fn additive(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.multiplicative()?;
        loop {
            let op = if self.is_op("+") {
                "+"
            } else if self.is_op("-") {
                "-"
            } else {
                break;
            };
            self.pos += 1;
            let right = self.multiplicative()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn multiplicative(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.special()?;
        loop {
            let op = if self.is_op("*") {
                "*"
            } else if self.is_op("/") {
                "/"
            } else {
                break;
            };
            self.pos += 1;
            let right = self.special()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn special(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.unary()?;
        while let Some(Tok::Special(s)) = self.peek().cloned() {
            self.pos += 1;
            let right = self.unary()?;
            left = Expr::Special(s, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<Expr, FormulaError> {
        if self.is_op("-") {
            self.pos += 1;
            let inner = self.unary()?;
            return Ok(Expr::Unary("-", Box::new(inner)));
        }
        if self.is_op("+") {
            self.pos += 1;
            return self.unary();
        }
        self.power()
    }
    fn power(&mut self) -> Result<Expr, FormulaError> {
        let base = self.atom()?;
        if self.is_op("^") {
            self.pos += 1;
            // The exponent may carry a sign, and ^ groups from the right: 2 ^ 3 ^ 2 is 2 ^ 9.
            let exponent = self.unary()?;
            return Ok(Expr::Binary("^", Box::new(base), Box::new(exponent)));
        }
        Ok(base)
    }
    fn atom(&mut self) -> Result<Expr, FormulaError> {
        let at = self.here();
        match self.take() {
            Some(Tok::Num(v)) => Ok(Expr::Num(v)),
            Some(Tok::Str(s)) => Ok(Expr::Str(s)),
            Some(Tok::Open) => {
                let inner = self.or()?;
                match self.take() {
                    Some(Tok::Close) => Ok(inner),
                    _ => err("a ( is not closed", Some(at)),
                }
            }
            Some(Tok::Name(name)) => {
                if matches!(self.peek(), Some(Tok::Open)) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if matches!(self.peek(), Some(Tok::Close)) {
                        self.pos += 1;
                        return Ok(Expr::Call(name, args, at));
                    }
                    loop {
                        args.push(self.or()?);
                        match self.take() {
                            Some(Tok::Comma) => {}
                            Some(Tok::Close) => break,
                            _ => return err(format!("the call of {name} is not closed with )"), Some(at)),
                        }
                    }
                    return Ok(Expr::Call(name, args, at));
                }
                Ok(match name.as_str() {
                    "TRUE" => Expr::Bool(true),
                    "FALSE" => Expr::Bool(false),
                    "NA" => Expr::Na,
                    "pi" => Expr::Num(std::f64::consts::PI),
                    "Inf" => Expr::Num(f64::INFINITY),
                    _ => Expr::Var(name, at),
                })
            }
            Some(Tok::Op(o)) => err(format!("'{o}' cannot start here; a value or a name is expected"), Some(at)),
            Some(Tok::Close) => err("a ) with no ( before it", Some(at)),
            Some(Tok::Comma) => err("a comma where a value is expected", Some(at)),
            Some(Tok::Special(s)) => err(format!("'{s}' needs a value before it"), Some(at)),
            None => err("the formula ends where a value is expected", Some(self.end)),
        }
    }
}

/// A formula, read and ready to be worked out for each case.
#[derive(Debug, Clone)]
pub struct Formula {
    expr: Expr,
}

/// Reads a formula. Nothing is worked out until it is given values.
pub fn parse(text: &str) -> Result<Formula, FormulaError> {
    let toks = tokens(text)?;
    if toks.is_empty() {
        return err("the formula is empty", Some(0));
    }
    let mut p = Parser { toks, pos: 0, end: text.chars().count() };
    let expr = p.or()?;
    if p.pos < p.toks.len() {
        let at = p.here();
        return err("something is left over after the formula; an operator may be missing between two values", Some(at));
    }
    check_calls(&expr)?;
    Ok(Formula { expr })
}

/// The functions the language has, with how many values each takes (None: any number from one).
const FUNCTIONS: &[(&str, Option<(usize, usize)>)] = &[
    ("abs", Some((1, 1))),
    ("sqrt", Some((1, 1))),
    ("exp", Some((1, 1))),
    ("log", Some((1, 2))),
    ("log2", Some((1, 1))),
    ("log10", Some((1, 1))),
    ("sin", Some((1, 1))),
    ("cos", Some((1, 1))),
    ("tan", Some((1, 1))),
    ("floor", Some((1, 1))),
    ("ceiling", Some((1, 1))),
    ("trunc", Some((1, 1))),
    ("round", Some((1, 2))),
    ("pmin", None),
    ("pmax", None),
    ("ifelse", Some((3, 3))),
    ("is.na", Some((1, 1))),
    ("as.numeric", Some((1, 1))),
    ("rowmean", None),
    ("rowsum", None),
    ("rowmin", None),
    ("rowmax", None),
    ("nmiss", None),
    ("c", None),
];

fn check_calls(e: &Expr) -> Result<(), FormulaError> {
    match e {
        Expr::Call(name, args, at) => {
            let Some((_, arity)) = FUNCTIONS.iter().find(|(n, _)| n == name) else {
                let names: Vec<&str> = FUNCTIONS.iter().map(|(n, _)| *n).filter(|n| *n != "c").collect();
                return err(format!("there is no function named {name}; the functions are {}", names.join(", ")), Some(*at));
            };
            if let Some((lo, hi)) = arity {
                if args.len() < *lo || args.len() > *hi {
                    return err(
                        format!("{name} takes {} value{}, and was given {}", if lo == hi { lo.to_string() } else { format!("{lo} or {hi}") }, if *hi == 1 { "" } else { "s" }, args.len()),
                        Some(*at),
                    );
                }
            } else if args.is_empty() {
                return err(format!("{name} needs at least one value"), Some(*at));
            }
            args.iter().try_for_each(check_calls)
        }
        Expr::Unary(_, a) => check_calls(a),
        Expr::Binary(_, a, b) | Expr::Special(_, a, b) => {
            check_calls(a)?;
            check_calls(b)
        }
        _ => Ok(()),
    }
}

impl Formula {
    /// The names of the variables the formula uses, in the order they first appear.
    pub fn variables(&self) -> Vec<String> {
        fn walk(e: &Expr, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
            match e {
                Expr::Var(n, _) => {
                    if seen.insert(n.clone()) {
                        out.push(n.clone());
                    }
                }
                Expr::Unary(_, a) => walk(a, seen, out),
                Expr::Binary(_, a, b) | Expr::Special(_, a, b) => {
                    walk(a, seen, out);
                    walk(b, seen, out);
                }
                Expr::Call(_, args, _) => args.iter().for_each(|a| walk(a, seen, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.expr, &mut BTreeSet::new(), &mut out);
        out
    }

    /// Works the formula out for one case, `get` giving the value of each variable (None for a name that is not a variable).
    pub fn eval(&self, get: &dyn Fn(&str) -> Option<Value>) -> Result<Value, FormulaError> {
        eval(&self.expr, get)
    }
}

// ---------------------------------------------------------------- working out

fn num_of(v: &Value, what: &str, at: usize) -> Result<Option<f64>, FormulaError> {
    match v {
        Value::Na => Ok(None),
        Value::Num(x) => Ok(Some(*x)),
        Value::Bool(b) => Ok(Some(f64::from(u8::from(*b)))),
        Value::Text(t) => err(format!("{what} is text (\"{t}\"), not a number"), Some(at)),
    }
}

fn bool_of(v: &Value, at: usize) -> Result<Option<bool>, FormulaError> {
    match v {
        Value::Na => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        Value::Num(x) => Ok(Some(*x != 0.0)),
        Value::Text(t) => err(format!("\"{t}\" is text, and cannot be true or false"), Some(at)),
    }
}

/// How a number is written when it is compared with a text.
fn show(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::Na => None,
        Value::Text(t) => Some(t.clone()),
        Value::Num(x) => Some(show(*x)),
        Value::Bool(b) => Some(if *b { "TRUE" } else { "FALSE" }.to_string()),
    }
}

/// R's `round(x, digits)` for digits of zero or more: the nearer of the two decimal candidates, and the one with an even last digit
/// when they are as near as each other.
pub fn r_round(x: f64, digits: u32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    if digits == 0 {
        return round_half_even(x);
    }
    // A number with fewer digits than asked for is already as rounded as it can be.
    let dig_x = if x == 0.0 { 0 } else { x.abs().log10().floor() as i32 + 1 };
    if digits as i32 + dig_x > 15 {
        return x;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let p10 = 10f64.powi(digits as i32);
    let x10 = p10 * x;
    let i10 = x10.floor();
    let (xd, xu) = (i10 / p10, x10.ceil() / p10);
    let (du, dd) = (xu - x, x - xd);
    let even = (i10 % 2.0) == 0.0;
    sign * if dd < du || (dd == du && even) { xd } else { xu }
}

fn round_half_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        // Halfway: take the even neighbour.
        let t = x.trunc();
        if t % 2.0 == 0.0 {
            t
        } else {
            r
        }
    } else {
        r
    }
}

fn arithmetic(op: &str, a: f64, b: f64) -> f64 {
    match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        "^" => {
            // R: x^0 and 1^y are 1 even when the other is missing or not a number; powf agrees for the numbers that reach here.
            if b == 0.0 || a == 1.0 {
                1.0
            } else {
                a.powf(b)
            }
        }
        _ => f64::NAN,
    }
}

fn eval(e: &Expr, get: &dyn Fn(&str) -> Option<Value>) -> Result<Value, FormulaError> {
    Ok(match e {
        Expr::Num(v) => Value::Num(*v),
        Expr::Str(s) => Value::Text(s.clone()),
        Expr::Bool(b) => Value::Bool(*b),
        Expr::Na => Value::Na,
        Expr::Var(name, at) => get(name).ok_or_else(|| FormulaError { message: format!("there is no variable named {name}"), at: Some(*at) })?,
        Expr::Unary(op, a) => {
            let v = eval(a, get)?;
            match *op {
                "!" => match bool_of(&v, 0)? {
                    Some(b) => Value::Bool(!b),
                    None => Value::Na,
                },
                _ => match num_of(&v, "the value after the minus sign", 0)? {
                    Some(x) => Value::Num(-x),
                    None => Value::Na,
                },
            }
        }
        Expr::Binary(op, a, b) => {
            let (x, y) = (eval(a, get)?, eval(b, get)?);
            match *op {
                "&" => {
                    let (p, q) = (bool_of(&x, 0)?, bool_of(&y, 0)?);
                    match (p, q) {
                        (Some(false), _) | (_, Some(false)) => Value::Bool(false),
                        (Some(true), Some(true)) => Value::Bool(true),
                        _ => Value::Na,
                    }
                }
                "|" => {
                    let (p, q) = (bool_of(&x, 0)?, bool_of(&y, 0)?);
                    match (p, q) {
                        (Some(true), _) | (_, Some(true)) => Value::Bool(true),
                        (Some(false), Some(false)) => Value::Bool(false),
                        _ => Value::Na,
                    }
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" => compare(op, &x, &y)?,
                _ => {
                    let (p, q) = (num_of(&x, "the value before the sign", 0)?, num_of(&y, "the value after the sign", 0)?);
                    match (p, q) {
                        (Some(p), Some(q)) => Value::Num(arithmetic(op, p, q)),
                        // R's power keeps its answer when only the base or only the exponent is missing and the other makes it certain.
                        (None, Some(q)) if *op == "^" && q == 0.0 => Value::Num(1.0),
                        (Some(p), None) if *op == "^" && p == 1.0 => Value::Num(1.0),
                        _ => Value::Na,
                    }
                }
            }
        }
        Expr::Special(op, a, b) => {
            let x = eval(a, get)?;
            if op == "%in%" {
                let set = match &**b {
                    Expr::Call(n, args, _) if n == "c" => args.iter().map(|a| eval(a, get)).collect::<Result<Vec<_>, _>>()?,
                    other => vec![eval(other, get)?],
                };
                let found = set.iter().any(|s| match (&x, s) {
                    (Value::Na, Value::Na) => true,
                    (Value::Na, _) | (_, Value::Na) => false,
                    _ => matches!(compare("==", &x, s), Ok(Value::Bool(true))),
                });
                return Ok(Value::Bool(found));
            }
            let y = eval(b, get)?;
            match (num_of(&x, "the value before the sign", 0)?, num_of(&y, "the value after the sign", 0)?) {
                (Some(p), Some(q)) => Value::Num(if op == "%%" { p - (p / q).floor() * q } else { (p / q).floor() }),
                _ => Value::Na,
            }
        }
        Expr::Call(name, args, at) => call(name, args, *at, get)?,
    })
}

fn compare(op: &str, x: &Value, y: &Value) -> Result<Value, FormulaError> {
    if matches!(x, Value::Na) || matches!(y, Value::Na) {
        return Ok(Value::Na);
    }
    let ordering = match (x, y) {
        (Value::Text(_), _) | (_, Value::Text(_)) => {
            let (a, b) = (text_of(x).unwrap_or_default(), text_of(y).unwrap_or_default());
            a.cmp(&b)
        }
        _ => {
            let (a, b) = (num_of(x, "the left value", 0)?.unwrap_or(f64::NAN), num_of(y, "the right value", 0)?.unwrap_or(f64::NAN));
            match a.partial_cmp(&b) {
                Some(o) => o,
                None => return Ok(Value::Na),
            }
        }
    };
    use std::cmp::Ordering::{Equal, Greater, Less};
    Ok(Value::Bool(match op {
        "==" => ordering == Equal,
        "!=" => ordering != Equal,
        "<" => ordering == Less,
        "<=" => ordering != Greater,
        ">" => ordering == Greater,
        _ => ordering != Less,
    }))
}

fn call(name: &str, args: &[Expr], at: usize, get: &dyn Fn(&str) -> Option<Value>) -> Result<Value, FormulaError> {
    let values: Vec<Value> = args.iter().map(|a| eval(a, get)).collect::<Result<_, _>>()?;
    let one = |f: &dyn Fn(f64) -> f64| -> Result<Value, FormulaError> {
        Ok(match num_of(&values[0], &format!("the value given to {name}"), at)? {
            Some(x) => Value::Num(f(x)),
            None => Value::Na,
        })
    };
    match name {
        "abs" => one(&f64::abs),
        "sqrt" => one(&f64::sqrt),
        "exp" => one(&f64::exp),
        "log2" => one(&f64::log2),
        "log10" => one(&f64::log10),
        "sin" => one(&f64::sin),
        "cos" => one(&f64::cos),
        "tan" => one(&f64::tan),
        "floor" => one(&f64::floor),
        "ceiling" => one(&f64::ceil),
        "trunc" => one(&f64::trunc),
        "log" => {
            let x = num_of(&values[0], "the value given to log", at)?;
            let base = if values.len() == 2 { num_of(&values[1], "the base given to log", at)? } else { Some(std::f64::consts::E) };
            Ok(match (x, base) {
                (Some(x), Some(b)) => Value::Num(if values.len() == 2 { x.ln() / b.ln() } else { x.ln() }),
                _ => Value::Na,
            })
        }
        "round" => {
            let x = num_of(&values[0], "the value given to round", at)?;
            let d = if values.len() == 2 { num_of(&values[1], "the number of digits given to round", at)? } else { Some(0.0) };
            match (x, d) {
                (Some(x), Some(d)) if d >= 0.0 && d.fract() == 0.0 && d <= 20.0 => Ok(Value::Num(r_round(x, d as u32))),
                (Some(_), Some(_)) => err("round takes a whole number of digits from 0 to 20", Some(at)),
                _ => Ok(Value::Na),
            }
        }
        "pmin" | "pmax" => {
            let mut best: Option<f64> = None;
            for v in &values {
                match num_of(v, &format!("a value given to {name}"), at)? {
                    None => return Ok(Value::Na),
                    Some(x) => best = Some(best.map_or(x, |b| if name == "pmin" { b.min(x) } else { b.max(x) })),
                }
            }
            Ok(best.map_or(Value::Na, Value::Num))
        }
        "ifelse" => Ok(match bool_of(&values[0], at)? {
            Some(true) => values[1].clone(),
            Some(false) => values[2].clone(),
            None => Value::Na,
        }),
        "is.na" => Ok(Value::Bool(match &values[0] {
            Value::Na => true,
            Value::Num(x) => x.is_nan(),
            _ => false,
        })),
        "as.numeric" => Ok(match &values[0] {
            Value::Na => Value::Na,
            Value::Num(x) => Value::Num(*x),
            Value::Bool(b) => Value::Num(f64::from(u8::from(*b))),
            Value::Text(t) => t.trim().parse::<f64>().map_or(Value::Na, Value::Num),
        }),
        "nmiss" => Ok(Value::Num(values.iter().filter(|v| matches!(v, Value::Na) || matches!(v, Value::Num(x) if x.is_nan())).count() as f64)),
        "rowmean" | "rowsum" | "rowmin" | "rowmax" => {
            let mut nums: Vec<f64> = Vec::new();
            for v in &values {
                match num_of(v, &format!("a value given to {name}"), at)? {
                    Some(x) if !x.is_nan() => nums.push(x),
                    _ => {}
                }
            }
            if nums.is_empty() {
                return Ok(Value::Na);
            }
            Ok(Value::Num(match name {
                "rowmean" => nums.iter().sum::<f64>() / nums.len() as f64,
                "rowsum" => nums.iter().sum::<f64>(),
                "rowmin" => nums.iter().copied().fold(f64::INFINITY, f64::min),
                _ => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            }))
        }
        "c" => err("c(...) is only for listing the values after %in%", Some(at)),
        _ => err(format!("there is no function named {name}"), Some(at)),
    }
}

/// Works a formula out for every case. `names` and `columns` are the variables (each column has one value for each case); a name the
/// formula uses that is not among them is an error that names it.
pub fn evaluate_columns(formula: &Formula, names: &[String], columns: &[Vec<Value>]) -> Result<Vec<Value>, FormulaError> {
    let n = columns.first().map_or(0, Vec::len);
    for used in formula.variables() {
        if !names.contains(&used) {
            let at = None;
            return err(format!("there is no variable named {used}"), at);
        }
    }
    let mut out = Vec::with_capacity(n);
    #[allow(clippy::needless_range_loop)] // a case is the same place in every column
    for row in 0..n {
        let get = |name: &str| names.iter().position(|n| n == name).map(|i| columns[i][row].clone());
        out.push(formula.eval(&get)?);
    }
    Ok(out)
}
