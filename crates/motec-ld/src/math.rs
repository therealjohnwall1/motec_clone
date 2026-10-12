//! B3: Excel-style math channels over decoded log data.
//!
//! Model: every expression evaluates to a SERIES over one output timeline.
//! Think of a spreadsheet where each row is a timestamp `t = i / rate_out`:
//! a channel reference is a "VLOOKUP in time" (linear interpolation of that
//! channel's native samples at `t`), and every operator/function applies
//! row-by-row. See `docs/MATHS_GUIDE.md` for the user guide.
//!
//! Rates: the output timeline defaults to the fastest referenced channel
//! (capped at 1000 Hz) and spans the longest input; both are overridable.
//! Inputs shorter than the timeline hold their last value (documented).

use std::collections::HashMap;
use std::fmt;

/// A named math channel definition (serializable intent; evaluation is separate).
#[derive(Debug, Clone)]
pub struct MathDef {
    pub name: String,
    pub expr: String,
    /// None = fastest referenced input. Some(hz) forces 1..=1000 Hz.
    pub rate_hz: Option<f64>,
    pub unit: String,
}

/// One resampleable input series: native samples + their rate.
pub struct MathInput<'a> {
    pub rate_hz: f64,
    pub data: &'a [f64],
}

/// Extra named constants usable inside expressions.
pub type Consts<'a> = HashMap<String, f64>;

#[derive(Debug, Clone)]
pub enum MathError {
    Parse(String),
    UnknownChannel { name: String, available: Vec<String> },
    UnknownFunction(String),
    Arity { func: String, expected: String, got: usize },
    NoInputs,
    BadRate(f64),
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::Parse(e) => write!(f, "parse error: {e}"),
            MathError::UnknownChannel { name, available } => {
                write!(f, "unknown channel '{name}'. available: {}", available.join(", "))
            }
            MathError::UnknownFunction(s) => write!(f, "unknown function '{s}'"),
            MathError::Arity { func, expected, got } => {
                write!(f, "'{func}' expects {expected} args, got {got}")
            }
            MathError::NoInputs => write!(f, "expression references no channels"),
            MathError::BadRate(r) => write!(f, "rate must be 1..=1000 Hz, got {r}"),
        }
    }
}

impl std::error::Error for MathError {}

// ---------------- output timeline ----------------

/// Choose (rate_hz, n_samples) for the output timeline.
pub fn plan_output<'a>(
    inputs: &HashMap<String, MathInput<'a>>,
    rate_opt: Option<f64>,
) -> Result<(f64, usize), MathError> {
    if inputs.is_empty() {
        // pure-constant expression (Excel: a bare formula is one cell)
        let rate = rate_opt.unwrap_or(1.0);
        if !(1.0..=1000.0).contains(&rate) {
            return Err(MathError::BadRate(rate));
        }
        return Ok((rate, 1));
    }
    let fastest = inputs.values().map(|i| i.rate_hz).fold(0.0f64, f64::max);
    let rate = rate_opt.unwrap_or(fastest);
    if !(1.0..=1000.0).contains(&rate) {
        return Err(MathError::BadRate(rate));
    }
    let dur = inputs
        .values()
        .map(|i| {
            if i.rate_hz > 0.0 && !i.data.is_empty() {
                (i.data.len() - 1) as f64 / i.rate_hz
            } else {
                0.0
            }
        })
        .fold(0.0f64, f64::max);
    Ok((rate, (dur * rate).ceil() as usize + 1))
}

/// Linear interpolation of one input at time t (seconds); holds last value past the end.
fn sample_at(input: &MathInput, t: f64) -> f64 {
    let d = input.data;
    if d.is_empty() || input.rate_hz <= 0.0 {
        return f64::NAN;
    }
    if t <= 0.0 {
        return d[0];
    }
    let pos = t * input.rate_hz;
    let lo = pos.floor() as usize;
    if lo >= d.len() - 1 {
        return d[d.len() - 1];
    }
    let frac = pos - lo as f64;
    d[lo] + (d[lo + 1] - d[lo]) * frac
}

// ---------------- lexer ----------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Chan(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
    Not,
}

fn lex(src: &str) -> Result<Vec<Tok>, MathError> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    let err = |pos: usize, msg: String| MathError::Parse(format!("{msg} at char {pos} in '{src}'"));
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '+' => { out.push(Tok::Plus); i += 1; }
            '-' => { out.push(Tok::Minus); i += 1; }
            '*' => { out.push(Tok::Star); i += 1; }
            '/' => { out.push(Tok::Slash); i += 1; }
            '^' => { out.push(Tok::Caret); i += 1; }
            '(' => { out.push(Tok::LParen); i += 1; }
            ')' => { out.push(Tok::RParen); i += 1; }
            ',' => { out.push(Tok::Comma); i += 1; }
            '=' => {
                if b.get(i + 1) == Some(&b'=') { out.push(Tok::Eq); i += 2; }
                else { return Err(err(i, "single '=' — did you mean '=='?".into())); }
            }
            '!' => {
                if b.get(i + 1) == Some(&b'=') { out.push(Tok::Ne); i += 2; }
                else { out.push(Tok::Not); i += 1; }
            }
            '>' => {
                if b.get(i + 1) == Some(&b'=') { out.push(Tok::Ge); i += 2; }
                else { out.push(Tok::Gt); i += 1; }
            }
            '<' => {
                if b.get(i + 1) == Some(&b'=') { out.push(Tok::Le); i += 2; }
                else { out.push(Tok::Lt); i += 1; }
            }
            '&' => {
                if b.get(i + 1) == Some(&b'&') { out.push(Tok::And); i += 2; }
                else { return Err(err(i, "single '&' — use '&&' or AND".into())); }
            }
            '|' => {
                if b.get(i + 1) == Some(&b'|') { out.push(Tok::Or); i += 2; }
                else { return Err(err(i, "single '|' — use '||' or OR".into())); }
            }
            '\'' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'\'' { j += 1; }
                if j >= b.len() { return Err(err(i, "unterminated channel name '...'".into())); }
                out.push(Tok::Chan(src[i + 1..j].to_string()));
                i = j + 1;
            }
            '0'..='9' | '.' => {
                let mut j = i;
                while j < b.len() && ((b[j] as char).is_ascii_digit() || b[j] == b'.' || b[j] == b'e' || b[j] == b'E' || ((b[j] == b'+' || b[j] == b'-') && j > i && (b[j-1] == b'e' || b[j-1] == b'E'))) { j += 1; }
                match src[i..j].parse::<f64>() {
                    Ok(v) => out.push(Tok::Num(v)),
                    Err(_) => return Err(err(i, format!("bad number '{}'", &src[i..j]))),
                }
                i = j;
            }
            'A'..='Z' | 'a'..='z' | '_' => {
                let mut j = i;
                while j < b.len() && ((b[j] as char).is_alphanumeric() || b[j] == b'_') { j += 1; }
                let word = &src[i..j];
                match word.to_ascii_uppercase().as_str() {
                    "AND" => out.push(Tok::And),
                    "OR" => out.push(Tok::Or),
                    "NOT" => out.push(Tok::Not),
                    _ => out.push(Tok::Ident(word.to_string())),
                }
                i = j;
            }
            _ => return Err(err(i, format!("unexpected character '{c}'"))),
        }
    }
    Ok(out)
}

// ---------------- AST ----------------

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Chan(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum BinOp { Add, Sub, Mul, Div, Pow, Eq, Ne, Gt, Lt, Ge, Le, And, Or }

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    src: String,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> { self.toks.get(self.pos) }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() { self.pos += 1; }
        t
    }
    fn err(&self, msg: &str) -> MathError {
        MathError::Parse(format!("{msg} near token {} of '{}'", self.pos, self.src))
    }
    fn parse(mut self) -> Result<Expr, MathError> {
        let e = self.parse_or()?;
        if self.pos != self.toks.len() {
            return Err(self.err("unexpected trailing input"));
        }
        Ok(e)
    }
    fn parse_or(&mut self) -> Result<Expr, MathError> {
        let mut e = self.parse_and()?;
        while matches!(self.peek(), Some(Tok::Or)) {
            self.next();
            e = Expr::Bin(BinOp::Or, Box::new(e), Box::new(self.parse_and()?));
        }
        Ok(e)
    }
    fn parse_and(&mut self) -> Result<Expr, MathError> {
        let mut e = self.parse_cmp()?;
        while matches!(self.peek(), Some(Tok::And)) {
            self.next();
            e = Expr::Bin(BinOp::And, Box::new(e), Box::new(self.parse_cmp()?));
        }
        Ok(e)
    }
    fn parse_cmp(&mut self) -> Result<Expr, MathError> {
        let mut e = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Eq) => BinOp::Eq, Some(Tok::Ne) => BinOp::Ne,
                Some(Tok::Gt) => BinOp::Gt, Some(Tok::Lt) => BinOp::Lt,
                Some(Tok::Ge) => BinOp::Ge, Some(Tok::Le) => BinOp::Le,
                _ => break,
            };
            self.next();
            e = Expr::Bin(op, Box::new(e), Box::new(self.parse_add()?));
        }
        Ok(e)
    }
    fn parse_add(&mut self) -> Result<Expr, MathError> {
        let mut e = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add, Some(Tok::Minus) => BinOp::Sub,
                _ => break,
            };
            self.next();
            e = Expr::Bin(op, Box::new(e), Box::new(self.parse_mul()?));
        }
        Ok(e)
    }
    fn parse_mul(&mut self) -> Result<Expr, MathError> {
        let mut e = self.parse_pow()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul, Some(Tok::Slash) => BinOp::Div,
                _ => break,
            };
            self.next();
            e = Expr::Bin(op, Box::new(e), Box::new(self.parse_pow()?));
        }
        Ok(e)
    }
    fn parse_pow(&mut self) -> Result<Expr, MathError> {
        let base = self.parse_unary()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.next();
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(self.parse_pow()?)));
        }
        Ok(base)
    }
    fn parse_unary(&mut self) -> Result<Expr, MathError> {
        match self.peek() {
            Some(Tok::Minus) => { self.next(); Ok(Expr::Neg(Box::new(self.parse_unary()?))) }
            Some(Tok::Not) => { self.next(); Ok(Expr::Not(Box::new(self.parse_unary()?))) }
            _ => self.parse_primary(),
        }
    }
    fn parse_primary(&mut self) -> Result<Expr, MathError> {
        match self.next() {
            Some(Tok::Num(v)) => Ok(Expr::Num(v)),
            Some(Tok::Chan(name)) => Ok(Expr::Chan(name)),
            Some(Tok::Ident(name)) => {
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.next();
                    let mut args = vec![];
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            args.push(self.parse_or()?);
                            if matches!(self.peek(), Some(Tok::Comma)) { self.next(); }
                            else { break; }
                        }
                    }
                    match self.next() {
                        Some(Tok::RParen) => Ok(Expr::Call(name, args)),
                        _ => Err(self.err("expected ')'")),
                    }
                } else {
                    // bare constant or unquoted single-word channel
                    Ok(Expr::Chan(name))
                }
            }
            Some(Tok::LParen) => {
                let e = self.parse_or()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err(self.err("expected ')'")),
                }
            }
            other => Err(self.err(&format!("unexpected {other:?}"))),
        }
    }
}

// ---------------- evaluation ----------------

struct Ctx<'a> {
    inputs: &'a HashMap<String, MathInput<'a>>,
    consts: &'a Consts<'a>,
    rate: f64,
    n: usize,
}

fn truth(x: f64) -> f64 { if x != 0.0 && !x.is_nan() { 1.0 } else { 0.0 } }

fn eval_const_or_chan<'a>(
    name: &str,
    ctx: &Ctx<'a>,
    available: &[String],
) -> Result<Vec<f64>, MathError> {
    if let Some(v) = ctx.consts.get(name) {
        return Ok(vec![*v; ctx.n]);
    }
    // case-insensitive channel lookup (MoTeC names are matched loosely)
    let key = ctx.inputs.keys().find(|k| k.eq_ignore_ascii_case(name));
    match key {
        Some(k) => {
            let inp = &ctx.inputs[k];
            Ok((0..ctx.n).map(|i| sample_at(inp, i as f64 / ctx.rate)).collect())
        }
        None => Err(MathError::UnknownChannel { name: name.to_string(), available: available.to_vec() }),
    }
}

fn eval(e: &Expr, ctx: &Ctx, available: &[String]) -> Result<Vec<f64>, MathError> {
    let n = ctx.n;
    Ok(match e {
        Expr::Num(v) => vec![*v; n],
        Expr::Chan(name) => eval_const_or_chan(name, ctx, available)?,
        Expr::Neg(x) => eval(x, ctx, available)?.into_iter().map(|v| -v).collect(),
        Expr::Not(x) => eval(x, ctx, available)?.into_iter().map(|v| 1.0 - truth(v)).collect(),
        Expr::Bin(op, a, b) => {
            let (x, y) = (eval(a, ctx, available)?, eval(b, ctx, available)?);
            x.into_iter().zip(y).map(|(a, b)| match op {
                BinOp::Add => a + b, BinOp::Sub => a - b, BinOp::Mul => a * b,
                BinOp::Div => a / b, BinOp::Pow => a.powf(b),
                BinOp::Eq => if a == b { 1.0 } else { 0.0 },
                BinOp::Ne => if a != b { 1.0 } else { 0.0 },
                BinOp::Gt => if a > b { 1.0 } else { 0.0 },
                BinOp::Lt => if a < b { 1.0 } else { 0.0 },
                BinOp::Ge => if a >= b { 1.0 } else { 0.0 },
                BinOp::Le => if a <= b { 1.0 } else { 0.0 },
                BinOp::And => truth(a) * truth(b),
                BinOp::Or => (truth(a) + truth(b) > 0.0) as u8 as f64,
            }).collect()
        }
        Expr::Call(f, args) => {
            let mut v: Vec<Vec<f64>> = Vec::with_capacity(args.len());
            for a in args { v.push(eval(a, ctx, available)?); }
            call(f, v)?
        }
    })
}

fn arity_err(f: &str, expected: &str, args: &[Vec<f64>]) -> Result<Vec<f64>, MathError> {
    Err(MathError::Arity { func: f.to_string(), expected: expected.to_string(), got: args.len() })
}

fn call(name: &str, args: Vec<Vec<f64>>) -> Result<Vec<f64>, MathError> {
    let one = |f: &str, args: &Vec<Vec<f64>>| -> Result<(), MathError> {
        if args.len() != 1 { arity_err(f, "1", args).map(|_| ()) } else { Ok(()) }
    };
    let two = |f: &str, args: &Vec<Vec<f64>>| -> Result<(), MathError> {
        if args.len() != 2 { arity_err(f, "2", args).map(|_| ()) } else { Ok(()) }
    };
    let el = |g: fn(f64) -> f64| args[0].iter().map(|&v| g(v)).collect::<Vec<_>>();
    match name.to_ascii_lowercase().as_str() {
        "abs" => { one(name, &args)?; Ok(el(f64::abs)) }
        "sqrt" => { one(name, &args)?; Ok(el(f64::sqrt)) }
        "sin" => { one(name, &args)?; Ok(el(f64::sin)) }
        "cos" => { one(name, &args)?; Ok(el(f64::cos)) }
        "tan" => { one(name, &args)?; Ok(el(f64::tan)) }
        "exp" => { one(name, &args)?; Ok(el(f64::exp)) }
        "ln" => { one(name, &args)?; Ok(el(f64::ln)) }
        "floor" => { one(name, &args)?; Ok(el(f64::floor)) }
        "ceil" => { one(name, &args)?; Ok(el(f64::ceil)) }
        "pow" => {
            two(name, &args)?;
            Ok(args[0].iter().zip(&args[1]).map(|(&a, &b)| a.powf(b)).collect())
        }
        "min" => {
            two(name, &args)?;
            Ok(args[0].iter().zip(&args[1]).map(|(&a, &b)| a.min(b)).collect())
        }
        "max" => {
            two(name, &args)?;
            Ok(args[0].iter().zip(&args[1]).map(|(&a, &b)| a.max(b)).collect())
        }
        "choose" => {
            if args.len() != 3 { return arity_err(name, "3 (cond, if_true, if_false)", &args); }
            Ok(args[0].iter().zip(&args[1]).zip(&args[2]).map(|((&c, &t), &f)| if truth(c) != 0.0 { t } else { f }).collect())
        }
        "smooth" => {
            if args.len() != 2 { return arity_err(name, "2 (series, window_samples)", &args); }
            let w = (args[1][0].round() as usize).clamp(1, args[0].len().max(1));
            let s = &args[0];
            let mut out = vec![0.0; s.len()];
            let mut acc = 0.0;
            for i in 0..s.len() {
                acc += s[i];
                if i >= w { acc -= s[i - w]; }
                out[i] = acc / (i + 1).min(w) as f64;
            }
            Ok(out)
        }
        _ => Err(MathError::UnknownFunction(name.to_string())),
    }
}

/// Evaluate `def.expr` over the output timeline.
///
/// `channels`: decoded series keyed by channel name.
/// `consts`: user constants. Returns (rate_hz, values).
pub fn eval_math(
    def: &MathDef,
    channels: &HashMap<String, MathInput>,
    consts: &Consts,
) -> Result<(f64, Vec<f64>), MathError> {
    let toks = lex(&def.expr)?;
    let ast = Parser { toks, pos: 0, src: def.expr.clone() }.parse()?;
    let (rate, n) = plan_output(channels, def.rate_hz)?;
    let available: Vec<String> = channels.keys().cloned().collect();
    let ctx = Ctx { inputs: channels, consts, rate, n };
    Ok((rate, eval(&ast, &ctx, &available)?))
}

/// Names of channels referenced by an expression (for rate planning / validation).
pub fn referenced_channels(expr: &str) -> Result<Vec<String>, MathError> {
    let toks = lex(expr)?;
    let ast = Parser { toks, pos: 0, src: expr.to_string() }.parse()?;
    let mut out = vec![];
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Chan(c) => { if !out.iter().any(|x| x == c) { out.push(c.clone()); } }
            Expr::Num(_) => {}
            Expr::Neg(x) | Expr::Not(x) => walk(x, out),
            Expr::Bin(_, a, b) => { walk(a, out); walk(b, out); }
            Expr::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
        }
    }
    walk(&ast, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp<'a>(rate: f64, data: &'a [f64]) -> MathInput<'a> { MathInput { rate_hz: rate, data } }

    fn run(expr: &str, ch: &HashMap<String, MathInput>, rate: Option<f64>) -> (f64, Vec<f64>) {
        let def = MathDef { name: "t".into(), expr: expr.into(), rate_hz: rate, unit: "".into() };
        eval_math(&def, ch, &Consts::new()).expect(expr)
    }

    #[test]
    fn arithmetic_and_precedence() {
        let ch = HashMap::new();
        let (_, v) = run("2 + 3 * 4 ^ 2", &ch, Some(10.0));
        assert_eq!(v.len(), 1);
        assert!((v[0] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_rates_resample() {
        // 10 Hz ramp 0..10 over 1 s + 1 Hz const 100; output 10 Hz
        let ramp: Vec<f64> = (0..=10).map(|i| i as f64).collect();
        let slow = vec![100.0, 100.0];
        let mut ch = HashMap::new();
        ch.insert("fast".into(), inp(10.0, &ramp));
        ch.insert("slow".into(), inp(1.0, &slow));
        let (rate, v) = run("'fast' + 'slow'", &ch, None);
        assert_eq!(rate, 10.0);
        assert_eq!(v.len(), 11);
        assert!((v[0] - 100.0).abs() < 1e-9);
        assert!((v[5] - 105.0).abs() < 1e-9);
        assert!((v[10] - 110.0).abs() < 1e-9);
    }

    #[test]
    fn short_input_holds_last() {
        let a = vec![1.0, 2.0]; // 1 Hz -> 1 s
        let b: Vec<f64> = (0..=20).map(|i| i as f64).collect(); // 10 Hz -> 2 s
        let mut ch = HashMap::new();
        ch.insert("a".into(), inp(1.0, &a));
        ch.insert("b".into(), inp(10.0, &b));
        let (_, v) = run("a + b", &ch, None);
        assert_eq!(v.len(), 21);
        assert!((v[20] - (2.0 + 20.0)).abs() < 1e-9); // a held at 2.0
    }

    #[test]
    fn choose_and_logic() {
        let x: Vec<f64> = vec![-1.0, 0.5, 2.0];
        let mut ch = HashMap::new();
        ch.insert("x".into(), inp(1.0, &x));
        let (_, v) = run("choose('x' > 0 AND 'x' < 2, 10, -10)", &ch, Some(1.0));
        // timeline: t=0,1,2 -> x interp: -1, 0.5, 2
        assert_eq!(v, vec![-10.0, 10.0, -10.0]);
    }

    #[test]
    fn smooth_and_funcs() {
        let x = vec![0.0, 0.0, 0.0, 10.0, 10.0];
        let mut ch = HashMap::new();
        ch.insert("x".into(), inp(1.0, &x));
        let (_, v) = run("smooth('x', 3) + sqrt(16)", &ch, Some(1.0));
        assert!((v[4] - (20.0 / 3.0 + 4.0)).abs() < 1e-9);
        assert!((v[3] - (10.0 / 3.0 + 4.0)).abs() < 1e-6);
    }

    #[test]
    fn unknown_channel_lists_available() {
        let mut ch = HashMap::new();
        ch.insert("TPS".into(), inp(10.0, &[1.0]));
        let def = MathDef { name: "t".into(), expr: "'Nope' + 1".into(), rate_hz: None, unit: "".into() };
        match eval_math(&def, &ch, &Consts::new()) {
            Err(MathError::UnknownChannel { available, .. }) => assert!(available.contains(&"TPS".to_string())),
            r => panic!("unexpected {r:?}"),
        }
    }

    #[test]
    fn kmh_example() {
        // Corr Speed m/s @ 100 Hz -> km/h
        let v: Vec<f64> = vec![0.0, 10.0, 27.78];
        let mut ch = HashMap::new();
        ch.insert("Corr Speed".into(), inp(100.0, &v));
        let (rate, out) = run("'Corr Speed' * 3.6", &ch, None);
        assert_eq!(rate, 100.0);
        assert!((out[2] - 100.008).abs() < 1e-6);
    }
}
