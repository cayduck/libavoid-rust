//! Interpreter for the fixture op scripts in `tests/fixtures/<name>.ops`.
//!
//! The op scripts are generated from the upstream C++ fixtures by
//! `tools/fixtures/convert.py` (see its module docstring for the format).
//! [`run_fixture`] executes one script against `libavoid_rust::avoid`:
//!
//! * every `output ROUTER GOLDEN` op compares `outputDiagramText()` with
//!   `tests/golden-bump/GOLDEN.txt` ([`assert_matches_golden_bump`]);
//! * `delete ROUTER GOLDEN` compares the final state (the oracle dumps it
//!   from the C++ `Router` destructor) and then calls `Router::destruct()`;
//! * `ret EXPR` is the fixture's `main()` return value and must equal the
//!   upstream exit code in `tests/golden/<fixture>.exit`;
//! * if the script prints (`print`, connector callbacks) its stdout must
//!   equal `tests/golden/<fixture>.stdout`.
//!
//! Scripts in `mode smoke` (fixtures that do not compile upstream) run the
//! same way but compare nothing.
//!
//! Errors in the script itself panic with a message starting with
//! `fixture-ops interpreter:`; panics raised by the library are annotated
//! (on stderr) with the op line being executed.

#![allow(dead_code)]
#![allow(clippy::uninlined_format_args)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use libavoid_rust::avoid::arena::{ClusterIdx, ConnIdx, PinIdx};
use libavoid_rust::avoid::viscluster::ClusterRef;
use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::hyperedge::HyperedgeNewAndDeletedObjectLists;
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::{Router, RoutingOption, RoutingParameter, POLY_LINE_ROUTING};
use libavoid_rust::avoid::shape::{ShapeRef, ShapeTransformationType};

use crate::common::{assert_matches_golden_bump, golden_exit_code, printf_f, read_golden_file};

// ---------------------------------------------------------------------------
// Script representation
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Expr {
    Atom(String),
    Str(String),
    Call(String, Vec<Expr>),
}

#[derive(Debug)]
enum Op {
    Let(String, String, Expr),
    Decl(String, String),
    Set(Expr, Expr),
    Do(Expr),
    Output { router: String, golden: String, checked: bool },
    Delete { router: String, golden: Option<String> },
    Callback(String, CbKind),
    Print(String),
    Assert(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Ret(Expr),
}

#[derive(Debug)]
struct Stmt {
    line: usize,
    text: String,
    op: Op,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CbKind {
    Route,
    DisplayRoute,
}

struct Script {
    fixture: String,
    golden_mode: bool,
    body: Vec<Stmt>,
    prints: bool,
}

fn script_err(name: &str, line: usize, msg: &str) -> ! {
    panic!("fixture-ops interpreter: {}.ops:{}: {}", name, line, msg)
}

fn tokenize(name: &str, line: usize, s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' || c == ')' {
            toks.push(c.to_string());
            i += 1;
        } else if c == '"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != '"' {
                j += if b[j] == '\\' { 2 } else { 1 };
            }
            if j >= b.len() {
                script_err(name, line, "unterminated string");
            }
            toks.push(b[i..=j].iter().collect());
            i = j + 1;
        } else {
            let mut j = i;
            while j < b.len() && !b[j].is_whitespace() && b[j] != '(' && b[j] != ')' {
                j += 1;
            }
            toks.push(b[i..j].iter().collect());
            i = j;
        }
    }
    toks
}

fn unescape_c(name: &str, line: usize, lit: &str) -> String {
    let inner = &lit[1..lit.len() - 1];
    let mut out = String::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                other => script_err(name, line, &format!("unsupported escape {:?}", other)),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_expr(name: &str, line: usize, toks: &[String], pos: &mut usize) -> Expr {
    let t = toks
        .get(*pos)
        .unwrap_or_else(|| script_err(name, line, "unexpected end of expression"));
    *pos += 1;
    if t == "(" {
        let f = toks
            .get(*pos)
            .unwrap_or_else(|| script_err(name, line, "unexpected end of expression"))
            .clone();
        *pos += 1;
        let mut args = Vec::new();
        loop {
            match toks.get(*pos) {
                Some(t) if t == ")" => {
                    *pos += 1;
                    break;
                }
                Some(_) => args.push(parse_expr(name, line, toks, pos)),
                None => script_err(name, line, "unbalanced parentheses"),
            }
        }
        Expr::Call(f, args)
    } else if t == ")" {
        script_err(name, line, "unexpected `)`")
    } else if t.starts_with('"') {
        Expr::Str(unescape_c(name, line, t))
    } else {
        Expr::Atom(t.clone())
    }
}

fn parse_full_expr(name: &str, line: usize, toks: &[String], start: usize) -> Expr {
    let mut pos = start;
    let e = parse_expr(name, line, toks, &mut pos);
    if pos != toks.len() {
        script_err(name, line, "trailing tokens");
    }
    e
}

fn parse_script(name: &str, text: &str) -> Script {
    let mut fixture = None;
    let mut golden_mode = None;
    let mut prints = false;
    // Stack of blocks being built (the bottom one is the script body).
    let mut stack: Vec<Block> = vec![Block::new(0, String::new(), None)];
    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        let toks = tokenize(name, line, s);
        let kw = toks[0].as_str();
        let want = |n: usize| {
            if toks.len() != n {
                script_err(name, line, &format!("`{}` expects {} tokens", kw, n));
            }
        };
        let op = match kw {
            "fixture" => {
                want(2);
                fixture = Some(toks[1].clone());
                continue;
            }
            "mode" => {
                want(2);
                golden_mode = Some(match toks[1].as_str() {
                    "golden" => true,
                    "smoke" => false,
                    m => script_err(name, line, &format!("unknown mode {}", m)),
                });
                continue;
            }
            "let" => Op::Let(toks[1].clone(), toks[2].clone(), parse_full_expr(name, line, &toks, 3)),
            "decl" => {
                want(3);
                Op::Decl(toks[1].clone(), toks[2].clone())
            }
            "set" => {
                let mut pos = 1;
                let lhs = parse_expr(name, line, &toks, &mut pos);
                Op::Set(lhs, parse_full_expr(name, line, &toks, pos))
            }
            "do" => Op::Do(parse_full_expr(name, line, &toks, 1)),
            "output" | "output-unchecked" => {
                want(3);
                Op::Output { router: toks[1].clone(), golden: toks[2].clone(), checked: kw == "output" }
            }
            "delete" => {
                want(3);
                Op::Delete {
                    router: toks[1].clone(),
                    golden: if toks[2] == "-" { None } else { Some(toks[2].clone()) },
                }
            }
            "callback" => {
                want(3);
                prints = true;
                Op::Callback(
                    toks[1].clone(),
                    match toks[2].as_str() {
                        "route" => CbKind::Route,
                        "displayRoute" => CbKind::DisplayRoute,
                        k => script_err(name, line, &format!("unknown callback kind {}", k)),
                    },
                )
            }
            "print" => {
                want(2);
                prints = true;
                if !toks[1].starts_with('"') {
                    script_err(name, line, "print expects a string");
                }
                Op::Print(unescape_c(name, line, &toks[1]))
            }
            "assert" => Op::Assert(parse_full_expr(name, line, &toks, 1)),
            "ret" => Op::Ret(parse_full_expr(name, line, &toks, 1)),
            "if" => {
                let cond = parse_full_expr(name, line, &toks, 1);
                stack.push(Block::new(line, s.to_string(), Some(cond)));
                continue;
            }
            "else" => {
                want(1);
                let top = stack.last_mut().unwrap();
                if top.cond.is_none() || top.in_else {
                    script_err(name, line, "unexpected else");
                }
                top.in_else = true;
                continue;
            }
            "end" => {
                want(1);
                if stack.len() < 2 {
                    script_err(name, line, "unexpected end");
                }
                let b = stack.pop().unwrap();
                stack.last_mut().unwrap().push_stmt(Stmt {
                    line: b.line,
                    text: b.text,
                    op: Op::If(b.cond.unwrap(), b.then, b.els),
                });
                continue;
            }
            k => script_err(name, line, &format!("unknown op `{}`", k)),
        };
        stack.last_mut().unwrap().push_stmt(Stmt { line, text: s.to_string(), op });
    }
    if stack.len() != 1 {
        script_err(name, 0, "unterminated if");
    }
    let fixture = fixture.unwrap_or_else(|| script_err(name, 0, "missing `fixture` header"));
    if fixture != name {
        script_err(name, 0, &format!("script is for fixture {}", fixture));
    }
    Script {
        fixture,
        golden_mode: golden_mode.unwrap_or_else(|| script_err(name, 0, "missing `mode` header")),
        body: stack.pop().unwrap().then,
        prints,
    }
}

/// A block being parsed: the script body or an `if` (with its `else`).
struct Block {
    then: Vec<Stmt>,
    els: Vec<Stmt>,
    in_else: bool,
    line: usize,
    text: String,
    cond: Option<Expr>,
}

impl Block {
    fn new(line: usize, text: String, cond: Option<Expr>) -> Block {
        Block { then: Vec::new(), els: Vec::new(), in_else: false, line, text, cond }
    }

    fn push_stmt(&mut self, s: Stmt) {
        if self.in_else {
            self.els.push(s)
        } else {
            self.then.push(s)
        }
    }
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Value {
    Void,
    Null,
    Int(i64),
    Double(f64),
    Bool(bool),
    Router,
    Rerouter,
    Poly(Polygon),
    Point(Point),
    ConnEnd(ConnEnd),
    Conn(ConnIdx),
    Shape(ShapeRef),
    Junction(JunctionRef),
    Pin(PinIdx),
    Cluster(ClusterIdx),
    Checkpoint(Checkpoint),
    Checkpoints(Vec<Checkpoint>),
    ConnEndList(Vec<ConnEnd>),
    NdLists(HyperedgeNewAndDeletedObjectLists),
    ConnList(Vec<ConnIdx>),
    JunctionList(Vec<JunctionRef>),
    JIter(Vec<JunctionRef>, usize),
    CIter(Vec<ConnIdx>, usize),
    JcPair(Option<JunctionRef>, Option<ConnIdx>),
    EndPair(ConnEnd, ConnEnd),
}

impl Value {
    fn kind(&self) -> &'static str {
        match self {
            Value::Void => "void",
            Value::Null => "null",
            Value::Int(_) => "int",
            Value::Double(_) => "double",
            Value::Bool(_) => "bool",
            Value::Router => "router",
            Value::Rerouter => "rerouter",
            Value::Poly(_) => "poly",
            Value::Point(_) => "point",
            Value::ConnEnd(_) => "connend",
            Value::Conn(_) => "conn",
            Value::Shape(_) => "shape",
            Value::Junction(_) => "junction",
            Value::Pin(_) => "pin",
            Value::Cluster(_) => "cluster",
            Value::Checkpoint(_) => "checkpoint",
            Value::Checkpoints(_) => "checkpoints",
            Value::ConnEndList(_) => "connendlist",
            Value::NdLists(_) => "ndlists",
            Value::ConnList(_) => "connlist",
            Value::JunctionList(_) => "junctionlist",
            Value::JIter(..) => "jiter",
            Value::CIter(..) => "citer",
            Value::JcPair(..) => "jcpair",
            Value::EndPair(..) => "endpair",
        }
    }
}

const POINTER_TYPES: &[&str] = &["router", "conn", "shape", "junction", "pin", "rerouter", "cluster"];

type CallbackLog = Rc<RefCell<Vec<(ConnIdx, CbKind)>>>;

struct Interp {
    name: String,
    router: Option<Router>,
    router_var: Option<String>,
    router_deleted: bool,
    vars: HashMap<String, (String, Value)>,
    stdout: String,
    log: CallbackLog,
    ret: Option<i64>,
    line: usize,
    golden_mode: bool,
    /// Interpreter self-test: poly-line router, nothing compared.
    dry_run: bool,
}

impl Interp {
    fn err(&self, msg: &str) -> ! {
        script_err(&self.name, self.line, msg)
    }

    fn r(&mut self) -> &mut Router {
        if self.router_deleted {
            self.err("use of deleted router");
        }
        match self.router.as_mut() {
            Some(r) => r,
            None => script_err(&self.name, self.line, "no router"),
        }
    }

    // --- conversions ---
    fn num(&self, v: &Value) -> f64 {
        match v {
            Value::Int(i) => *i as f64,
            Value::Double(d) => *d,
            Value::Bool(b) => *b as i64 as f64,
            _ => self.err(&format!("expected a number, got {}", v.kind())),
        }
    }
    fn int(&self, v: &Value) -> i64 {
        match v {
            Value::Int(i) => *i,
            Value::Bool(b) => *b as i64,
            _ => self.err(&format!("expected an integer, got {}", v.kind())),
        }
    }
    fn uint(&self, v: &Value) -> u32 {
        let i = self.int(v);
        if !(0..=u32::MAX as i64).contains(&i) {
            self.err(&format!("integer {} out of range for unsigned", i));
        }
        i as u32
    }
    fn boolean(&self, v: &Value) -> bool {
        match v {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Double(d) => *d != 0.0,
            _ => self.err(&format!("expected a bool, got {}", v.kind())),
        }
    }
    fn point(&self, v: &Value) -> Point {
        match v {
            Value::Point(p) => *p,
            _ => self.err(&format!("expected a point, got {}", v.kind())),
        }
    }
    fn poly(&self, v: &Value) -> Polygon {
        match v {
            Value::Poly(p) => p.clone(),
            _ => self.err(&format!("expected a polygon, got {}", v.kind())),
        }
    }
    fn connend(&self, v: &Value) -> ConnEnd {
        match v {
            Value::ConnEnd(c) => c.clone(),
            _ => self.err(&format!("expected a ConnEnd, got {}", v.kind())),
        }
    }
    fn conn(&self, v: &Value) -> ConnIdx {
        match v {
            Value::Conn(c) => *c,
            Value::Null => self.err("null ConnRef dereferenced"),
            _ => self.err(&format!("expected a ConnRef, got {}", v.kind())),
        }
    }
    fn shape(&self, v: &Value) -> ShapeRef {
        match v {
            Value::Shape(s) => *s,
            Value::Null => self.err("null ShapeRef dereferenced"),
            _ => self.err(&format!("expected a ShapeRef, got {}", v.kind())),
        }
    }
    fn junction(&self, v: &Value) -> JunctionRef {
        match v {
            Value::Junction(j) => *j,
            Value::Null => self.err("null JunctionRef dereferenced"),
            _ => self.err(&format!("expected a JunctionRef, got {}", v.kind())),
        }
    }
    fn router_arg(&self, v: &Value) {
        if !matches!(v, Value::Router) {
            self.err(&format!("expected the router, got {}", v.kind()));
        }
    }
    fn param(&self, v: &Value) -> RoutingParameter {
        let i = self.int(v);
        *RoutingParameter::ALL
            .get(i as usize)
            .unwrap_or_else(|| self.err(&format!("bad RoutingParameter {}", i)))
    }
    fn option(&self, v: &Value) -> RoutingOption {
        let i = self.int(v);
        *RoutingOption::ALL
            .get(i as usize)
            .unwrap_or_else(|| self.err(&format!("bad RoutingOption {}", i)))
    }

    /// Converts `v` to the declared op type `ty` (C++ implicit conversions
    /// between arithmetic types; null for pointers).
    fn coerce(&self, ty: &str, v: Value) -> Value {
        match (ty, &v) {
            ("int", Value::Int(_)) | ("double", Value::Double(_)) | ("bool", Value::Bool(_)) => v,
            ("int", Value::Bool(b)) => Value::Int(*b as i64),
            ("int", Value::Double(d)) => Value::Int(d.trunc() as i64),
            ("double", Value::Int(i)) => Value::Double(*i as f64),
            ("double", Value::Bool(b)) => Value::Double(*b as i64 as f64),
            ("bool", _) => Value::Bool(self.boolean(&v)),
            (t, Value::Null) if POINTER_TYPES.contains(&t) => v,
            (t, _) if t == v.kind() => v,
            ("junctionlist", Value::JunctionList(_)) => v,
            _ => self.err(&format!("cannot store {} in a variable of type {}", v.kind(), ty)),
        }
    }

    fn default_value(&self, ty: &str) -> Value {
        match ty {
            "connend" => Value::ConnEnd(ConnEnd::new()),
            "poly" => Value::Poly(Polygon::new()),
            "checkpoints" => Value::Checkpoints(Vec::new()),
            "connendlist" => Value::ConnEndList(Vec::new()),
            "ndlists" => Value::NdLists(HyperedgeNewAndDeletedObjectLists::default()),
            "jiter" => Value::JIter(Vec::new(), 0),
            "citer" => Value::CIter(Vec::new(), 0),
            "point" => Value::Point(Point::default()),
            _ => self.err(&format!("no default value for type {}", ty)),
        }
    }

    fn var(&self, name: &str) -> Value {
        match self.vars.get(name) {
            Some((_, v)) => v.clone(),
            None => self.err(&format!("unknown variable {}", name)),
        }
    }

    fn var_mut(&mut self, name: &str) -> &mut Value {
        if !self.vars.contains_key(name) {
            self.err(&format!("unknown variable {}", name));
        }
        &mut self.vars.get_mut(name).unwrap().1
    }

    // --- evaluation ---
    fn eval(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Str(_) => self.err("string value in expression"),
            Expr::Atom(a) => self.atom(a),
            Expr::Call(f, args) => self.call(f, args),
        }
    }

    fn atom(&self, a: &str) -> Value {
        match a {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            "null" => return Value::Null,
            _ => {}
        }
        let first = a.as_bytes()[0];
        if first.is_ascii_digit() || first == b'-' || first == b'.' {
            if let Ok(i) = a.parse::<i64>() {
                return Value::Int(i);
            }
            if let Ok(d) = a.parse::<f64>() {
                return Value::Double(d);
            }
            self.err(&format!("bad numeric literal {}", a));
        }
        if a == self.router_var.as_deref().unwrap_or("") {
            if self.router_deleted {
                self.err("use of deleted router");
            }
            return Value::Router;
        }
        self.var(a)
    }

    fn arity(&self, f: &str, args: &[Expr], n: usize) {
        if args.len() != n {
            self.err(&format!("{} expects {} arguments, got {}", f, n, args.len()));
        }
    }

    fn call(&mut self, f: &str, args: &[Expr]) -> Value {
        // Mutating functions whose first argument is a variable.
        match f {
            "setPoint" | "resize" | "pushBack" => {
                let name = match &args[0] {
                    Expr::Atom(n) => n.clone(),
                    _ => self.err(&format!("{} on a temporary", f)),
                };
                let rest: Vec<Value> = args[1..].iter().map(|a| self.eval(a)).collect();
                let rest_kinds: Vec<&str> = rest.iter().map(|v| v.kind()).collect();
                let idx0 = if f == "pushBack" { None } else { rest.first().map(|v| self.int(v)) };
                let target = self.var_mut(&name);
                match (f, target, rest.as_slice()) {
                    ("setPoint", Value::Poly(p), [_, Value::Point(pt)]) => {
                        let i = idx0.unwrap() as usize;
                        if i >= p.ps.len() {
                            panic!("fixture-ops interpreter: setPoint index {} out of range", i);
                        }
                        p.set_point(i, pt);
                    }
                    ("resize", Value::Poly(p), [_]) => {
                        p.ps.resize(idx0.unwrap() as usize, Point::default());
                    }
                    ("pushBack", Value::Checkpoints(v), [Value::Checkpoint(c)]) => v.push(*c),
                    ("pushBack", Value::ConnEndList(v), [Value::ConnEnd(c)]) => v.push(c.clone()),
                    (_, t, _) => {
                        let k = t.kind();
                        self.err(&format!("{} on {} with {:?}", f, k, rest_kinds))
                    }
                }
                return Value::Void;
            }
            _ => {}
        }
        let v: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
        let n = |i: usize| -> &Value { &v[i] };
        macro_rules! ar {
            ($k:expr) => {
                self.arity(f, args, $k)
            };
        }
        match f {
            // --- arithmetic / logic ---
            "+" | "-" | "*" | "/" | "%" | "|" | "&" | "<" | ">" | "<=" | ">=" | "==" | "!=" => {
                ar!(2);
                self.binop(f, n(0), n(1))
            }
            "and" => {
                ar!(2);
                Value::Bool(self.boolean(n(0)) && self.boolean(n(1)))
            }
            "or" => {
                ar!(2);
                Value::Bool(self.boolean(n(0)) || self.boolean(n(1)))
            }
            "not" => {
                ar!(1);
                Value::Bool(!self.boolean(n(0)))
            }
            "neg" => {
                ar!(1);
                match n(0) {
                    Value::Int(i) => Value::Int(-i),
                    Value::Double(d) => Value::Double(-d),
                    o => self.err(&format!("neg of {}", o.kind())),
                }
            }
            "?" => {
                ar!(3);
                if self.boolean(n(0)) {
                    v[1].clone()
                } else {
                    v[2].clone()
                }
            }
            "int" | "double" | "bool" => {
                ar!(1);
                self.coerce(f, v[0].clone())
            }
            "fabs" => {
                ar!(1);
                Value::Double(self.num(n(0)).abs())
            }
            // --- value construction ---
            "Router" => {
                ar!(1);
                if self.router.is_some() {
                    self.err("a second Router is not supported by the interpreter");
                }
                let flags = if self.dry_run { POLY_LINE_ROUTING } else { self.uint(n(0)) };
                self.router = Some(Router::new(flags));
                Value::Router
            }
            "Point" => {
                ar!(2);
                Value::Point(Point::new(self.num(n(0)), self.num(n(1))))
            }
            "Polygon" => {
                ar!(1);
                Value::Poly(Polygon::with_size(self.int(n(0)) as usize))
            }
            "Rectangle" => {
                ar!(2);
                Value::Poly(Rectangle::new(&self.point(n(0)), &self.point(n(1))))
            }
            "ConnEnd" => {
                ar!(0);
                Value::ConnEnd(ConnEnd::new())
            }
            "ConnEndPt1" => {
                ar!(1);
                Value::ConnEnd(ConnEnd::from_point(self.point(n(0))))
            }
            "ConnEndPt" => {
                ar!(2);
                Value::ConnEnd(ConnEnd::from_point_dirs(self.point(n(0)), self.uint(n(1))))
            }
            "ConnEndShape" => {
                ar!(2);
                let (s, c) = (self.shape(n(0)), self.uint(n(1)));
                Value::ConnEnd(ConnEnd::from_shape_pin(self.r(), s, c))
            }
            "ConnEndJunction" => {
                ar!(1);
                let j = self.junction(n(0));
                Value::ConnEnd(ConnEnd::from_junction(self.r(), j))
            }
            "Checkpoint" => {
                ar!(1);
                Value::Checkpoint(Checkpoint::new(self.point(n(0))))
            }
            "CheckpointDirs" => {
                ar!(3);
                Value::Checkpoint(Checkpoint::with_directions(self.point(n(0)), self.uint(n(1)), self.uint(n(2))))
            }
            "Checkpoints" => {
                ar!(1);
                Value::Checkpoints(vec![Checkpoint::default(); self.int(n(0)) as usize])
            }
            // --- object creation ---
            "ShapeRef" => {
                ar!(3);
                self.router_arg(n(0));
                let (p, id) = (self.poly(n(1)), self.uint(n(2)));
                Value::Shape(ShapeRef::new(self.r(), &p, id))
            }
            "JunctionRef" => {
                ar!(3);
                self.router_arg(n(0));
                let (p, id) = (self.point(n(1)), self.uint(n(2)));
                Value::Junction(JunctionRef::new(self.r(), p, id))
            }
            "ConnRef" => {
                ar!(2);
                self.router_arg(n(0));
                let id = self.uint(n(1));
                Value::Conn(ConnRef::new(self.r(), id))
            }
            "ConnRefEnds" => {
                ar!(4);
                self.router_arg(n(0));
                let (s, d, id) = (self.connend(n(1)), self.connend(n(2)), self.uint(n(3)));
                Value::Conn(ConnRef::new_with_ends(self.r(), &s, &d, id))
            }
            "Pin" => {
                ar!(7);
                let s = self.shape(n(0));
                let (cls, x, y, prop) = (self.uint(n(1)), self.num(n(2)), self.num(n(3)), self.boolean(n(4)));
                let (inside, dirs) = (self.num(n(5)), self.uint(n(6)));
                Value::Pin(ShapeConnectionPin::new_shape(self.r(), s, cls, x, y, prop, inside, dirs))
            }
            "PinOld" => {
                ar!(6);
                let s = self.shape(n(0));
                let (cls, x, y) = (self.uint(n(1)), self.num(n(2)), self.num(n(3)));
                let (inside, dirs) = (self.num(n(4)), self.uint(n(5)));
                Value::Pin(ShapeConnectionPin::new_shape_proportional(self.r(), s, cls, x, y, inside, dirs))
            }
            "PinJunction" => {
                ar!(3);
                let j = self.junction(n(0));
                let (cls, dirs) = (self.uint(n(1)), self.uint(n(2)));
                Value::Pin(ShapeConnectionPin::new_junction(self.r(), j, cls, dirs))
            }
            // --- audit additions (tests/audit_scenes) ---
            "ClusterRef" => {
                ar!(3);
                self.router_arg(n(0));
                let (p, id) = (self.poly(n(1)), self.uint(n(2)));
                Value::Cluster(ClusterRef::new(self.r(), &p, id))
            }
            "deleteCluster" => {
                ar!(2);
                self.router_arg(n(0));
                let c = match n(1) {
                    Value::Cluster(c) => *c,
                    o => self.err(&format!("expected a ClusterRef, got {}", o.kind())),
                };
                self.r().delete_cluster(c);
                Value::Void
            }
            "clusterSetNewPoly" => {
                ar!(2);
                let c = match n(0) {
                    Value::Cluster(c) => *c,
                    o => self.err(&format!("expected a ClusterRef, got {}", o.kind())),
                };
                let p = self.poly(n(1));
                c.set_new_poly(self.r(), &p);
                Value::Void
            }
            "setRouterFlag" => {
                ar!(3);
                self.router_arg(n(0));
                let (flag, b) = (self.int(n(1)), self.boolean(n(2)));
                let r = self.r();
                match flag {
                    0 => r.partial_time = b,
                    1 => r.simple_routing = b,
                    2 => r.clustered_routing = b,
                    3 => r.ignore_regions = b,
                    4 => r.use_lees_algorithm = b,
                    5 => r.invisibility_grph = b,
                    6 => r.selective_reroute = b,
                    7 => r.partial_feedback = b,
                    8 => r.rubber_band_routing = b,
                    _ => self.err(&format!("bad router flag {}", flag)),
                }
                Value::Void
            }
            "markAllObstaclesAsMoved" => {
                ar!(1);
                self.router_arg(n(0));
                self.r().mark_all_obstacles_as_moved();
                Value::Void
            }
            "setFixedExistingRoute" => {
                ar!(1);
                let c = self.conn(n(0));
                c.set_fixed_existing_route(self.r());
                Value::Void
            }
            "setHateCrossings" => {
                ar!(2);
                let (c, b) = (self.conn(n(0)), self.boolean(n(1)));
                c.set_hate_crossings(self.r(), b);
                Value::Void
            }
            "setEndpoint" => {
                ar!(3);
                let (c, t, e) = (self.conn(n(0)), self.uint(n(1)), self.connend(n(2)));
                c.set_endpoint(self.r(), t, &e);
                Value::Void
            }
            "setCentrePos" => {
                ar!(2);
                let (sh, p) = (self.shape(n(0)), self.point(n(1)));
                sh.set_centre_pos(self.r(), &p);
                Value::Void
            }
            "preferOrthogonalDimension" => {
                ar!(2);
                let (j, d) = (self.junction(n(0)), self.uint(n(1)));
                j.prefer_orthogonal_dimension(self.r(), d as usize);
                Value::Void
            }
            "setConnectionCost" => {
                ar!(2);
                let p = match n(0) {
                    Value::Pin(p) => *p,
                    o => self.err(&format!("expected a pin, got {}", o.kind())),
                };
                let c = self.num(n(1));
                p.set_connection_cost(self.r(), c);
                Value::Void
            }
            // --- Router methods ---
            "processTransaction" => {
                ar!(1);
                self.router_arg(n(0));
                Value::Bool(self.r().process_transaction())
            }
            "setTransactionUse" => {
                ar!(2);
                self.router_arg(n(0));
                let b = self.boolean(n(1));
                self.r().set_transaction_use(b);
                Value::Void
            }
            "setRoutingParameter" | "setRoutingPenalty" => {
                ar!(3);
                self.router_arg(n(0));
                let (p, val) = (self.param(n(1)), self.num(n(2)));
                if f == "setRoutingParameter" {
                    self.r().set_routing_parameter(p, val);
                } else {
                    self.r().set_routing_penalty(p, val);
                }
                Value::Void
            }
            "setRoutingOption" => {
                ar!(3);
                self.router_arg(n(0));
                let (o, b) = (self.option(n(1)), self.boolean(n(2)));
                self.r().set_routing_option(o, b);
                Value::Void
            }
            "moveShape" => {
                ar!(4);
                self.router_arg(n(0));
                let (s, dx, dy) = (self.shape(n(1)), self.num(n(2)), self.num(n(3)));
                self.r().move_shape_by(s, dx, dy);
                Value::Void
            }
            "moveShapePoly" => {
                ar!(4);
                self.router_arg(n(0));
                let (s, p, first) = (self.shape(n(1)), self.poly(n(2)), self.boolean(n(3)));
                self.r().move_shape(s, &p, first);
                Value::Void
            }
            "moveJunction" => {
                ar!(4);
                self.router_arg(n(0));
                let (j, dx, dy) = (self.junction(n(1)), self.num(n(2)), self.num(n(3)));
                self.r().move_junction_by(j, dx, dy);
                Value::Void
            }
            "moveJunctionPt" => {
                ar!(3);
                self.router_arg(n(0));
                let (j, p) = (self.junction(n(1)), self.point(n(2)));
                self.r().move_junction(j, &p);
                Value::Void
            }
            "deleteShape" => {
                ar!(2);
                self.router_arg(n(0));
                let s = self.shape(n(1));
                self.r().delete_shape(s);
                Value::Void
            }
            "deleteConnector" => {
                ar!(2);
                self.router_arg(n(0));
                let c = self.conn(n(1));
                self.r().delete_connector(c);
                Value::Void
            }
            "deleteJunction" => {
                ar!(2);
                self.router_arg(n(0));
                let j = self.junction(n(1));
                self.r().delete_junction(j);
                Value::Void
            }
            "existsOrthogonalSegmentOverlap" => {
                ar!(2);
                self.router_arg(n(0));
                let b = self.boolean(n(1));
                Value::Bool(self.r().exists_orthogonal_segment_overlap(b))
            }
            "existsOrthogonalFixedSegmentOverlap" => {
                ar!(2);
                self.router_arg(n(0));
                let b = self.boolean(n(1));
                Value::Bool(self.r().exists_orthogonal_fixed_segment_overlap(b))
            }
            "existsOrthogonalTouchingPaths" => {
                ar!(1);
                self.router_arg(n(0));
                Value::Bool(self.r().exists_orthogonal_touching_paths())
            }
            "existsCrossings" => {
                ar!(2);
                self.router_arg(n(0));
                let b = self.boolean(n(1));
                Value::Int(self.r().exists_crossings(b) as i64)
            }
            "existsInvalidOrthogonalPaths" => {
                ar!(1);
                self.router_arg(n(0));
                Value::Bool(self.r().exists_invalid_orthogonal_paths())
            }
            "hyperedgeRerouter" => {
                ar!(1);
                self.router_arg(n(0));
                Value::Rerouter
            }
            "registerHyperedgeForReroutingJunction" => {
                ar!(2);
                let j = self.junction(n(1));
                Value::Int(self.r().hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j) as i64)
            }
            "registerHyperedgeForReroutingEnds" => {
                ar!(2);
                let list = match n(1) {
                    Value::ConnEndList(l) => l.clone(),
                    o => self.err(&format!("expected a ConnEndList, got {}", o.kind())),
                };
                Value::Int(self.r().hyperedge_rerouter().register_hyperedge_for_rerouting(list) as i64)
            }
            "newAndDeletedObjectListsFromHyperedgeImprovement" => {
                ar!(1);
                self.router_arg(n(0));
                Value::NdLists(self.r().new_and_deleted_object_lists_from_hyperedge_improvement())
            }
            // --- ConnRef methods ---
            "setSourceEndpoint" | "setDestEndpoint" => {
                ar!(2);
                let (c, e) = (self.conn(n(0)), self.connend(n(1)));
                if f == "setSourceEndpoint" {
                    c.set_source_endpoint(self.r(), &e);
                } else {
                    c.set_dest_endpoint(self.r(), &e);
                }
                Value::Void
            }
            "setEndpoints" => {
                ar!(3);
                let (c, s, d) = (self.conn(n(0)), self.connend(n(1)), self.connend(n(2)));
                c.set_endpoints(self.r(), &s, &d);
                Value::Void
            }
            "setRoutingType" => {
                ar!(2);
                let c = self.conn(n(0));
                let t = match self.int(n(1)) {
                    0 => ConnType::ConnTypeNone,
                    1 => ConnType::ConnTypePolyLine,
                    2 => ConnType::ConnTypeOrthogonal,
                    t => self.err(&format!("bad ConnType {}", t)),
                };
                c.set_routing_type(self.r(), t);
                Value::Void
            }
            "setRoutingCheckpoints" => {
                ar!(2);
                let c = self.conn(n(0));
                let cps = match n(1) {
                    Value::Checkpoints(v) => v.clone(),
                    o => self.err(&format!("expected checkpoints, got {}", o.kind())),
                };
                c.set_routing_checkpoints(self.r(), &cps);
                Value::Void
            }
            "setFixedRoute" => {
                ar!(2);
                let (c, p) = (self.conn(n(0)), self.poly(n(1)));
                c.set_fixed_route(self.r(), &p);
                Value::Void
            }
            "clearFixedRoute" => {
                ar!(1);
                let c = self.conn(n(0));
                c.clear_fixed_route(self.r());
                Value::Void
            }
            "makePathInvalid" => {
                ar!(1);
                let c = self.conn(n(0));
                self.r().conns[c].make_path_invalid();
                Value::Void
            }
            "displayRoute" => {
                ar!(1);
                let c = self.conn(n(0));
                Value::Poly(c.display_route(self.r()).clone())
            }
            "route" => {
                ar!(1);
                let c = self.conn(n(0));
                Value::Poly(c.route(self.r()).clone())
            }
            "splitAtSegment" => {
                ar!(2);
                let (c, seg) = (self.conn(n(0)), self.int(n(1)));
                let (j, nc) = c.split_at_segment(self.r(), seg as usize);
                Value::JcPair(j, nc)
            }
            "endpointConnEnds" => {
                ar!(1);
                let c = self.conn(n(0));
                let (a, b) = c.endpoint_conn_ends(self.r());
                Value::EndPair(a, b)
            }
            "id" => {
                ar!(1);
                let id = match n(0) {
                    Value::Conn(c) => c.id(self.r()),
                    Value::Shape(s) => s.id(self.r()),
                    Value::Junction(j) => j.id(self.r()),
                    o => self.err(&format!("id() of {}", o.kind())),
                };
                Value::Int(id as i64)
            }
            // --- ShapeRef / JunctionRef / pin methods ---
            "transformConnectionPinPositions" => {
                ar!(2);
                let s = self.shape(n(0));
                let t = match self.int(n(1)) {
                    0 => ShapeTransformationType::TransformationTypeCW90,
                    1 => ShapeTransformationType::TransformationTypeCW180,
                    2 => ShapeTransformationType::TransformationTypeCW270,
                    3 => ShapeTransformationType::TransformationTypeFlipX,
                    4 => ShapeTransformationType::TransformationTypeFlipY,
                    t => self.err(&format!("bad ShapeTransformationType {}", t)),
                };
                s.transform_connection_pin_positions(self.r(), t);
                Value::Void
            }
            "setPositionFixed" => {
                ar!(2);
                let (j, b) = (self.junction(n(0)), self.boolean(n(1)));
                j.set_position_fixed(self.r(), b);
                Value::Void
            }
            "recommendedPosition" => {
                ar!(1);
                let j = self.junction(n(0));
                Value::Point(j.recommended_position(self.r()))
            }
            "position" => {
                ar!(1);
                let j = self.junction(n(0));
                Value::Point(j.position(self.r()))
            }
            "attachedConnectors" => {
                ar!(1);
                let j = self.junction(n(0));
                Value::ConnList(j.attached_connectors(self.r()))
            }
            "removeJunctionAndMergeConnectors" => {
                ar!(1);
                let j = self.junction(n(0));
                match j.remove_junction_and_merge_connectors(self.r()) {
                    Some(c) => Value::Conn(c),
                    None => Value::Null,
                }
            }
            "setExclusive" => {
                ar!(2);
                let p = match n(0) {
                    Value::Pin(p) => *p,
                    o => self.err(&format!("setExclusive on {}", o.kind())),
                };
                let b = self.boolean(n(1));
                p.set_exclusive(self.r(), b);
                Value::Void
            }
            "shape" | "junction" => {
                ar!(1);
                let e = self.connend(n(0));
                if f == "shape" {
                    match e.shape(self.r()) {
                        Some(s) => Value::Shape(s),
                        None => Value::Null,
                    }
                } else {
                    match e.junction(self.r()) {
                        Some(j) => Value::Junction(j),
                        None => Value::Null,
                    }
                }
            }
            // --- plain data accessors ---
            "size" => {
                ar!(1);
                let len = match n(0) {
                    Value::Poly(p) => p.ps.len(),
                    Value::ConnList(l) => l.len(),
                    Value::JunctionList(l) => l.len(),
                    Value::Checkpoints(l) => l.len(),
                    o => self.err(&format!("size() of {}", o.kind())),
                };
                Value::Int(len as i64)
            }
            "ps" => {
                ar!(2);
                let (p, i) = (self.poly(n(0)), self.int(n(1)));
                match p.ps.get(i as usize) {
                    Some(pt) if i >= 0 => Value::Point(*pt),
                    _ => self.err(&format!("ps[{}] out of range (size {})", i, p.ps.len())),
                }
            }
            "at" => {
                ar!(2);
                let i = self.int(n(1));
                match n(0) {
                    Value::Checkpoints(v) if i >= 0 && (i as usize) < v.len() => Value::Checkpoint(v[i as usize]),
                    o => self.err(&format!("[{}] on {}", i, o.kind())),
                }
            }
            "x" | "y" => {
                ar!(1);
                let p = self.point(n(0));
                Value::Double(if f == "x" { p.x } else { p.y })
            }
            "_id" => {
                ar!(1);
                Value::Int(self.poly(n(0))._id as i64)
            }
            "first" | "second" => {
                ar!(1);
                let first = f == "first";
                match n(0) {
                    Value::JcPair(j, c) => {
                        if first {
                            j.map(Value::Junction).unwrap_or(Value::Null)
                        } else {
                            c.map(Value::Conn).unwrap_or(Value::Null)
                        }
                    }
                    Value::EndPair(a, b) => Value::ConnEnd(if first { a.clone() } else { b.clone() }),
                    o => self.err(&format!("{} of {}", f, o.kind())),
                }
            }
            "newJunctionList" | "deletedJunctionList" => {
                ar!(1);
                match n(0) {
                    Value::NdLists(l) => Value::JunctionList(if f == "newJunctionList" {
                        l.new_junction_list.clone()
                    } else {
                        l.deleted_junction_list.clone()
                    }),
                    o => self.err(&format!("{} of {}", f, o.kind())),
                }
            }
            "newConnectorList" | "deletedConnectorList" | "changedConnectorList" => {
                ar!(1);
                match n(0) {
                    Value::NdLists(l) => Value::ConnList(match f {
                        "newConnectorList" => l.new_connector_list.clone(),
                        "deletedConnectorList" => l.deleted_connector_list.clone(),
                        _ => l.changed_connector_list.clone(),
                    }),
                    o => self.err(&format!("{} of {}", f, o.kind())),
                }
            }
            "begin" | "end" => {
                ar!(1);
                match n(0) {
                    Value::JunctionList(l) => Value::JIter(l.clone(), if f == "begin" { 0 } else { l.len() }),
                    Value::ConnList(l) => Value::CIter(l.clone(), if f == "begin" { 0 } else { l.len() }),
                    o => self.err(&format!("{}() of {}", f, o.kind())),
                }
            }
            "deref" => {
                ar!(1);
                match n(0) {
                    Value::JIter(l, i) if *i < l.len() => Value::Junction(l[*i]),
                    Value::CIter(l, i) if *i < l.len() => Value::Conn(l[*i]),
                    o => self.err(&format!("dereference of past-the-end/invalid {}", o.kind())),
                }
            }
            "next" => {
                ar!(1);
                match n(0) {
                    Value::JIter(l, i) if *i < l.len() => Value::JIter(l.clone(), i + 1),
                    Value::CIter(l, i) if *i < l.len() => Value::CIter(l.clone(), i + 1),
                    o => self.err(&format!("increment of past-the-end/invalid {}", o.kind())),
                }
            }
            _ => self.err(&format!("unknown function `{}`", f)),
        }
    }

    fn binop(&self, op: &str, a: &Value, b: &Value) -> Value {
        // Pointer / iterator equality.
        let ptr_eq = match (a, b) {
            (Value::Null, Value::Null) => Some(true),
            (Value::Null, x) | (x, Value::Null) if POINTER_TYPES.contains(&x.kind()) => Some(false),
            (Value::Conn(x), Value::Conn(y)) => Some(x == y),
            (Value::Shape(x), Value::Shape(y)) => Some(x == y),
            (Value::Junction(x), Value::Junction(y)) => Some(x == y),
            (Value::Pin(x), Value::Pin(y)) => Some(x == y),
            (Value::JIter(l1, i1), Value::JIter(l2, i2)) => Some(l1 == l2 && i1 == i2),
            (Value::CIter(l1, i1), Value::CIter(l2, i2)) => Some(l1 == l2 && i1 == i2),
            _ => None,
        };
        if let Some(eq) = ptr_eq {
            return match op {
                "==" => Value::Bool(eq),
                "!=" => Value::Bool(!eq),
                _ => self.err(&format!("operator {} on pointers", op)),
            };
        }
        let is_num = |v: &Value| matches!(v, Value::Int(_) | Value::Double(_) | Value::Bool(_));
        if !is_num(a) || !is_num(b) {
            self.err(&format!("operator {} on {} and {}", op, a.kind(), b.kind()));
        }
        let both_int = !matches!(a, Value::Double(_)) && !matches!(b, Value::Double(_));
        if both_int {
            let (x, y) = (self.int(a), self.int(b));
            return match op {
                "+" => Value::Int(x + y),
                "-" => Value::Int(x - y),
                "*" => Value::Int(x * y),
                "/" | "%" => {
                    if y == 0 {
                        self.err("integer division by zero");
                    }
                    // Rust integer division truncates toward zero like C++.
                    Value::Int(if op == "/" { x / y } else { x % y })
                }
                "|" => Value::Int(x | y),
                "&" => Value::Int(x & y),
                "<" => Value::Bool(x < y),
                ">" => Value::Bool(x > y),
                "<=" => Value::Bool(x <= y),
                ">=" => Value::Bool(x >= y),
                "==" => Value::Bool(x == y),
                "!=" => Value::Bool(x != y),
                _ => self.err(&format!("operator {}", op)),
            };
        }
        let (x, y) = (self.num(a), self.num(b));
        match op {
            "+" => Value::Double(x + y),
            "-" => Value::Double(x - y),
            "*" => Value::Double(x * y),
            "/" => Value::Double(x / y),
            "<" => Value::Bool(x < y),
            ">" => Value::Bool(x > y),
            "<=" => Value::Bool(x <= y),
            ">=" => Value::Bool(x >= y),
            "==" => Value::Bool(x == y),
            "!=" => Value::Bool(x != y),
            _ => self.err(&format!("operator {} on doubles", op)),
        }
    }

    fn assign(&mut self, lhs: &Expr, rhs: Value) {
        match lhs {
            Expr::Atom(name) => {
                let ty = match self.vars.get(name) {
                    Some((t, _)) => t.clone(),
                    None => self.err(&format!("assignment to undeclared {}", name)),
                };
                let v = self.coerce(&ty, rhs);
                self.vars.get_mut(name).unwrap().1 = v;
            }
            Expr::Call(f, args) => {
                let name = match args.first() {
                    Some(Expr::Atom(n)) => n.clone(),
                    _ => self.err("unsupported assignment target"),
                };
                let idx = if args.len() > 1 {
                    let iv = self.eval(&args[1]);
                    Some(self.int(&iv))
                } else {
                    None
                };
                if args.len() != if matches!(f.as_str(), "ps" | "at") { 2 } else { 1 } {
                    self.err("bad assignment target arity");
                }
                let rhs_num = match &rhs {
                    Value::Int(_) | Value::Double(_) | Value::Bool(_) => Some(self.num(&rhs)),
                    _ => None,
                };
                let rhs_kind = rhs.kind();
                let line = self.line;
                let fixture = self.name.clone();
                let fail = move |m: String| -> ! { script_err(&fixture, line, &m) };
                let target = self.var_mut(&name);
                match (f.as_str(), target, rhs) {
                    ("ps", Value::Poly(p), Value::Point(pt)) => {
                        let i = idx.unwrap();
                        if i < 0 || i as usize >= p.ps.len() {
                            fail(format!("ps[{}] out of range (size {})", i, p.ps.len()));
                        }
                        p.ps[i as usize] = pt;
                    }
                    ("at", Value::Checkpoints(v), Value::Checkpoint(c)) => {
                        let i = idx.unwrap();
                        if i < 0 || i as usize >= v.len() {
                            fail(format!("[{}] out of range (size {})", i, v.len()));
                        }
                        v[i as usize] = c;
                    }
                    ("x", Value::Point(p), _) if rhs_num.is_some() => p.x = rhs_num.unwrap(),
                    ("y", Value::Point(p), _) if rhs_num.is_some() => p.y = rhs_num.unwrap(),
                    ("_id", Value::Poly(p), Value::Int(i)) => p._id = i as i32,
                    (f, t, _) => fail(format!("cannot assign {} to {} of {}", rhs_kind, f, t.kind())),
                }
            }
            Expr::Str(_) => self.err("cannot assign to a string"),
        }
    }

    fn flush_callbacks(&mut self) {
        let pending: Vec<(ConnIdx, CbKind)> = self.log.borrow_mut().drain(..).collect();
        for (conn, kind) in pending {
            let id = conn.id(self.r());
            self.stdout.push_str(&format!("Connector {} needs rerouting!\n", id));
            let route = match kind {
                CbKind::Route => conn.route(self.r()).clone(),
                CbKind::DisplayRoute => conn.display_route(self.r()).clone(),
            };
            self.stdout.push_str("New path: ");
            for i in 0..route.ps.len() {
                self.stdout.push_str(&format!(
                    "{}({}, {})",
                    if i > 0 { "-" } else { "" },
                    printf_f(route.ps[i].x),
                    printf_f(route.ps[i].y)
                ));
            }
            self.stdout.push('\n');
        }
    }

    fn exec_block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            if self.ret.is_some() {
                self.err("statement after ret");
            }
            self.line = s.line;
            let r = panic::catch_unwind(AssertUnwindSafe(|| self.exec(s)));
            if let Err(payload) = r {
                eprintln!("fixture-ops: panic while executing {}.ops:{}: `{}`", self.name, s.line, s.text);
                panic::resume_unwind(payload);
            }
            if !matches!(s.op, Op::If(..)) {
                self.flush_callbacks_if_any();
            }
        }
    }

    fn flush_callbacks_if_any(&mut self) {
        if !self.log.borrow().is_empty() {
            self.flush_callbacks();
        }
    }

    fn exec(&mut self, s: &Stmt) {
        match &s.op {
            Op::Let(name, ty, e) => {
                if self.vars.contains_key(name) {
                    self.err(&format!("redeclaration of {}", name));
                }
                let v = self.eval(e);
                if ty == "router" {
                    if !matches!(v, Value::Router) {
                        self.err("router variable must be initialised with (Router ..)");
                    }
                    self.router_var = Some(name.clone());
                    return;
                }
                let v = self.coerce(ty, v);
                self.vars.insert(name.clone(), (ty.clone(), v));
            }
            Op::Decl(name, ty) => {
                if self.vars.contains_key(name) {
                    self.err(&format!("redeclaration of {}", name));
                }
                let v = self.default_value(ty);
                self.vars.insert(name.clone(), (ty.clone(), v));
            }
            Op::Set(lhs, e) => {
                let v = self.eval(e);
                self.assign(lhs, v);
            }
            Op::Do(e) => {
                self.eval(e);
            }
            Op::Output { router, golden, checked } => {
                self.check_router_var(router);
                if self.golden_mode && *checked && !self.dry_run {
                    self.compare_output(golden);
                } else {
                    // Still call it: displayRoute() caches routes, so the
                    // call has side effects upstream too.
                    let _ = self.r().output_diagram_text();
                }
            }
            Op::Delete { router, golden } => {
                self.check_router_var(router);
                match golden {
                    Some(g) if self.golden_mode && !self.dry_run => {
                        self.compare_output(g);
                    }
                    Some(_) if !self.golden_mode && !is_audit(&self.name) => self.err("golden name in smoke mode"),
                    Some(_) => {
                        let _ = self.r().output_diagram_text();
                    }
                    None => {
                        if self.golden_mode {
                            self.err("delete without golden name in golden mode");
                        }
                        let _ = self.r().output_diagram_text();
                    }
                }
                self.r().destruct();
                self.router_deleted = true;
            }
            Op::Callback(conn, kind) => {
                let c = match self.var(conn) {
                    Value::Conn(c) => c,
                    o => self.err(&format!("callback on {}", o.kind())),
                };
                let log = self.log.clone();
                let kind = *kind;
                c.set_callback(self.r(), Some(Box::new(move |conn| log.borrow_mut().push((conn, kind)))));
            }
            Op::Print(text) => self.stdout.push_str(text),
            Op::Assert(e) => {
                let v = self.eval(e);
                if !self.boolean(&v) && !self.dry_run {
                    panic!(
                        "fixture {}: assertion from upstream fixture failed: {} (op line {})",
                        self.name, s.text, s.line
                    );
                }
            }
            Op::If(cond, then, els) => {
                let c = self.eval(cond);
                if self.boolean(&c) {
                    self.exec_block(then);
                } else {
                    self.exec_block(els);
                }
            }
            Op::Ret(e) => {
                let v = self.eval(e);
                self.ret = Some(self.int(&v));
            }
        }
    }

    /// Compares the router's diagram with the golden: `tests/golden-bump/`
    /// for upstream fixtures (within the fixture tolerance), and
    /// `tests/audit_scenes/golden/` -- bit for bit -- for the audit scenes.
    fn compare_output(&mut self, golden: &str) {
        if is_audit(&self.name) {
            let text = self.r().output_diagram_text();
            let expected = audit_golden(golden, "txt");
            if text != expected {
                let line = text.lines().zip(expected.lines()).position(|(a, b)| a != b).unwrap_or(0);
                panic!(
                    "audit scene {}: diagram {} differs from the C++ at line {}:\n  rust: {}\n  c++:  {}",
                    self.name,
                    golden,
                    line + 1,
                    text.lines().nth(line).unwrap_or("<none>"),
                    expected.lines().nth(line).unwrap_or("<none>")
                );
            }
        } else {
            assert_matches_golden_bump(self.r(), golden);
        }
    }

    fn check_router_var(&self, router: &str) {
        if self.router_var.as_deref() != Some(router) {
            self.err(&format!("{} is not the router", router));
        }
    }
}

/// Audit scenes (`tests/audit_scenes/audit_NNN.ops`, generated by
/// `tools/oracle/audit_scenes.py`, goldens by `tools/oracle/audit_scenes.sh`).
pub fn is_audit(name: &str) -> bool {
    name.starts_with("audit_")
}

/// Reads `tests/audit_scenes/golden/<name>.<ext>`.
pub fn audit_golden(name: &str, ext: &str) -> String {
    let path = format!("{}/tests/audit_scenes/golden/{}.{}", env!("CARGO_MANIFEST_DIR"), name, ext);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read audit golden {}: {}", path, e))
}

/// Path of `tests/fixtures/<name>.ops` (`tests/audit_scenes/<name>.ops` for
/// audit scenes).
pub fn ops_path(name: &str) -> String {
    if is_audit(name) {
        return format!("{}/tests/audit_scenes/{}.ops", env!("CARGO_MANIFEST_DIR"), name);
    }
    format!("{}/tests/fixtures/{}.ops", env!("CARGO_MANIFEST_DIR"), name)
}

/// Result of running an op script.
pub struct FixtureRun {
    /// `main()`'s return value.
    pub exit_code: i64,
    /// What the fixture printed itself (callbacks, printf).
    pub stdout: String,
    /// False for smoke-mode scripts (nothing was compared).
    pub golden_mode: bool,
}

/// Runs `tests/fixtures/<name>.ops`, comparing every output with the
/// goldens (in golden mode).  Panics on any mismatch.
pub fn run_fixture(name: &str) -> FixtureRun {
    run_fixture_impl(name, false)
}

/// Interpreter self-test: executes the whole script with the router forced
/// to `PolyLineRouting` (so that it does not reach unported orthogonal code)
/// and compares nothing (assertions, exit code and stdout are not checked).
/// The routes are of course meaningless; this only exercises every op of
/// the script against the real library API.
pub fn dry_run_fixture(name: &str) -> FixtureRun {
    run_fixture_impl(name, true)
}

/// Runs a golden-mode script without comparing anything (used for audit
/// scenes on which the C++ has undefined behaviour: the port must still not
/// panic).
pub fn smoke_run_fixture(name: &str) -> FixtureRun {
    let path = ops_path(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let mut script = parse_script(name, &text);
    script.golden_mode = false;
    run_script(name, script, false)
}

fn run_fixture_impl(name: &str, dry_run: bool) -> FixtureRun {
    let path = ops_path(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let script = parse_script(name, &text);
    run_script(name, script, dry_run)
}

fn run_script(name: &str, script: Script, dry_run: bool) -> FixtureRun {
    let mut it = Interp {
        name: name.to_string(),
        router: None,
        router_var: None,
        router_deleted: false,
        vars: HashMap::new(),
        stdout: String::new(),
        log: Rc::default(),
        ret: None,
        line: 0,
        golden_mode: script.golden_mode,
        dry_run,
    };
    it.exec_block(&script.body);
    let exit_code = it.ret.unwrap_or_else(|| script_err(name, 0, "script ended without ret"));
    if script.golden_mode && !dry_run {
        let expected = if is_audit(name) {
            audit_golden(&script.fixture, "exit").trim().parse().expect("exit code")
        } else {
            golden_exit_code(&script.fixture)
        };
        assert_eq!(
            (exit_code & 0xff) as i32,
            expected,
            "fixture {}: main() returned {} but upstream exited with {}",
            name,
            exit_code,
            expected
        );
        if script.prints {
            let golden = if is_audit(name) {
                audit_golden(&script.fixture, "stdout")
            } else {
                read_golden_file(&script.fixture, "stdout")
            };
            assert_eq!(it.stdout, golden, "fixture {}: stdout differs from upstream", name);
        }
    }
    // Leaving the router undeleted (improveHyperedge05 never deletes it)
    // mirrors upstream, where it is leaked.
    FixtureRun { exit_code, stdout: it.stdout, golden_mode: script.golden_mode }
}
