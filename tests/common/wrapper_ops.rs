//! Runs the fixture op scripts in `tests/fixtures/<name>.ops` (see
//! `fixture_ops.rs`) through the JS-facing string-id wrapper
//! [`libavoid_rust::AvoidRouter`] instead of the core API, translating
//! every upstream call into the corresponding wrapper method (shapes,
//! junctions, connectors get string ids `s1`, `j2`, `c3`, ... and the
//! fixture's numeric ids are passed as `numericId`).  Every `output` /
//! `delete` dump (`outputDiagramText(true)`) and every connector's
//! `displayConnectorRoute()` JSON are compared with
//! `tests/golden-bump/`, exit codes with `tests/golden/<f>.exit` and
//! (where the fixture prints) stdout with `tests/golden/<f>.stdout`.

#![allow(dead_code)]
#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::AvoidRouter;

use crate::json::{parse, J};
use crate::common::{compare_with_golden_in, golden_exit_code, printf_f, read_golden_file, read_golden_file_in, GoldenDir};

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

/// A ConnEnd in the wrapper's raw encoding.
#[derive(Clone, Debug)]
struct End {
    kind: f64,
    object: String,
    x: f64,
    y: f64,
    num: f64,
}

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
    End(Option<End>),
    Conn(String),
    Shape(String),
    Junction(String),
    /// (owner id, pin id, owner is a junction)
    Pin(String, String, bool),
    Checkpoint((f64, f64, u32, u32)),
    Checkpoints(Vec<(f64, f64, u32, u32)>),
    EndList(Vec<End>),
    NdLists(J),
    ConnList(Vec<String>),
    JunctionList(Vec<String>),
    JIter(Vec<String>, usize),
    CIter(Vec<String>, usize),
    JcPair(Option<String>, Option<String>),
    EndPair(J, J),
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
            Value::End(_) => "connend",
            Value::Conn(_) => "conn",
            Value::Shape(_) => "shape",
            Value::Junction(_) => "junction",
            Value::Pin(..) => "pin",
            Value::Checkpoint(_) => "checkpoint",
            Value::Checkpoints(_) => "checkpoints",
            Value::EndList(_) => "connendlist",
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

const POINTER_TYPES: &[&str] = &["router", "conn", "shape", "junction", "pin", "rerouter"];

struct Interp {
    name: String,
    router: Option<AvoidRouter>,
    router_var: Option<String>,
    router_deleted: bool,
    vars: HashMap<String, (String, Value)>,
    stdout: String,
    callbacks: HashMap<String, CbKind>,
    ret: Option<i64>,
    line: usize,
    golden_mode: bool,
    counter: usize,
    outputs_checked: usize,
}

fn flat(p: &Polygon) -> Vec<f64> {
    p.ps.iter().flat_map(|q| [q.x, q.y]).collect()
}

fn args(e: &End) -> (f64, String, String, f64, f64, f64) {
    (e.kind, e.object.clone(), String::new(), e.x, e.y, e.num)
}

impl Interp {
    fn err(&self, msg: &str) -> ! {
        script_err(&self.name, self.line, msg)
    }

    fn r(&mut self) -> &mut AvoidRouter {
        if self.router_deleted {
            self.err("use of deleted router");
        }
        match self.router.as_mut() {
            Some(r) => r,
            None => script_err(&self.name, self.line, "no router"),
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{}{}", prefix, self.counter)
    }

    fn check(&self, ok: bool, what: &str) {
        if !ok {
            panic!("fixture {} (wrapper) line {}: wrapper call {} failed", self.name, self.line, what);
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
    fn end(&self, v: &Value) -> End {
        match v {
            Value::End(Some(e)) => e.clone(),
            Value::End(None) => self.err("empty ConnEnd used"),
            _ => self.err(&format!("expected a ConnEnd, got {}", v.kind())),
        }
    }
    fn conn(&self, v: &Value) -> String {
        match v {
            Value::Conn(c) => c.clone(),
            _ => self.err(&format!("expected a ConnRef, got {}", v.kind())),
        }
    }
    fn shape(&self, v: &Value) -> String {
        match v {
            Value::Shape(s) => s.clone(),
            _ => self.err(&format!("expected a ShapeRef, got {}", v.kind())),
        }
    }
    fn junction(&self, v: &Value) -> String {
        match v {
            Value::Junction(j) => j.clone(),
            _ => self.err(&format!("expected a JunctionRef, got {}", v.kind())),
        }
    }

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
            _ => self.err(&format!("cannot store {} in a variable of type {}", v.kind(), ty)),
        }
    }

    fn default_value(&self, ty: &str) -> Value {
        match ty {
            "connend" => Value::End(None),
            "poly" => Value::Poly(Polygon::new()),
            "checkpoints" => Value::Checkpoints(Vec::new()),
            "connendlist" => Value::EndList(Vec::new()),
            "point" => Value::Point(Point::default()),
            "ndlists" => Value::NdLists(J::Null),
            "jiter" => Value::JIter(Vec::new(), 0),
            "citer" => Value::CIter(Vec::new(), 0),
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
            return Value::Router;
        }
        self.var(a)
    }

    fn json_end(&self, j: &J) -> Value {
        match j.get("type").str() {
            "point" => {
                let p = j.get("position");
                Value::End(Some(End {
                    kind: 0.0,
                    object: String::new(),
                    x: p.get("x").num(),
                    y: p.get("y").num(),
                    num: j.get("directions").num(),
                }))
            }
            "shapePin" => Value::End(Some(End {
                kind: 1.0,
                object: j.get("shapeId").str().to_string(),
                x: 0.0,
                y: 0.0,
                num: j.get("classId").num(),
            })),
            "junction" => Value::End(Some(End {
                kind: 2.0,
                object: j.get("junctionId").str().to_string(),
                x: 0.0,
                y: 0.0,
                num: 0.0,
            })),
            _ => Value::End(None),
        }
    }

    fn call(&mut self, f: &str, args_e: &[Expr]) -> Value {
        if let "setPoint" | "resize" | "pushBack" = f {
            let name = match &args_e[0] {
                Expr::Atom(n) => n.clone(),
                _ => self.err(&format!("{} on a temporary", f)),
            };
            let rest: Vec<Value> = args_e[1..].iter().map(|a| self.eval(a)).collect();
            let idx0 = if f == "pushBack" { None } else { rest.first().map(|v| self.int(v)) };
            let target = self.var_mut(&name);
            match (f, target, rest.as_slice()) {
                ("setPoint", Value::Poly(p), [_, Value::Point(pt)]) => p.ps[idx0.unwrap() as usize] = *pt,
                ("resize", Value::Poly(p), [_]) => p.ps.resize(idx0.unwrap() as usize, Point::default()),
                ("pushBack", Value::Checkpoints(v), [Value::Checkpoint(c)]) => v.push(*c),
                ("pushBack", Value::EndList(v), [Value::End(Some(c))]) => v.push(c.clone()),
                _ => panic!("fixture-ops interpreter: bad {}", f),
            }
            return Value::Void;
        }
        let v: Vec<Value> = args_e.iter().map(|a| self.eval(a)).collect();
        let n = |i: usize| -> &Value { &v[i] };
        match f {
            "+" | "-" | "*" | "/" | "%" | "|" | "&" | "<" | ">" | "<=" | ">=" | "==" | "!=" => self.binop(f, n(0), n(1)),
            "and" => Value::Bool(self.boolean(n(0)) && self.boolean(n(1))),
            "or" => Value::Bool(self.boolean(n(0)) || self.boolean(n(1))),
            "not" => Value::Bool(!self.boolean(n(0))),
            "neg" => match n(0) {
                Value::Int(i) => Value::Int(-i),
                Value::Double(d) => Value::Double(-d),
                o => self.err(&format!("neg of {}", o.kind())),
            },
            "?" => {
                if self.boolean(n(0)) {
                    v[1].clone()
                } else {
                    v[2].clone()
                }
            }
            "int" | "double" | "bool" => self.coerce(f, v[0].clone()),
            "fabs" => Value::Double(self.num(n(0)).abs()),
            "Router" => {
                let flags = self.int(n(0)) as u32;
                self.router = Some(AvoidRouter::create(flags).expect("valid router flags"));
                Value::Router
            }
            "Point" => Value::Point(Point::new(self.num(n(0)), self.num(n(1)))),
            "Polygon" => Value::Poly(Polygon::with_size(self.int(n(0)) as usize)),
            "Rectangle" => Value::Poly(Rectangle::new(&self.point(n(0)), &self.point(n(1)))),
            "ConnEnd" => Value::End(None),
            "ConnEndPt1" | "ConnEndPt" => {
                let p = self.point(n(0));
                let dirs = if f == "ConnEndPt" { self.num(n(1)) } else { 15.0 };
                Value::End(Some(End { kind: 0.0, object: String::new(), x: p.x, y: p.y, num: dirs }))
            }
            "ConnEndShape" => {
                let s = self.shape(n(0));
                Value::End(Some(End { kind: 1.0, object: s, x: 0.0, y: 0.0, num: self.num(n(1)) }))
            }
            "ConnEndJunction" => {
                let j = self.junction(n(0));
                Value::End(Some(End { kind: 2.0, object: j, x: 0.0, y: 0.0, num: 0.0 }))
            }
            "Checkpoint" => {
                let p = self.point(n(0));
                Value::Checkpoint((p.x, p.y, 15, 15))
            }
            "CheckpointDirs" => {
                let p = self.point(n(0));
                Value::Checkpoint((p.x, p.y, self.int(n(1)) as u32, self.int(n(2)) as u32))
            }
            "Checkpoints" => Value::Checkpoints(vec![(0.0, 0.0, 15, 15); self.int(n(0)) as usize]),
            "ShapeRef" => {
                let (p, id) = (self.poly(n(1)), self.num(n(2)));
                let name = self.fresh("s");
                let ok = self.r().add_polygon_shape(name.clone(), flat(&p), Some(id));
                self.check(ok, "addPolygonShape");
                Value::Shape(name)
            }
            "JunctionRef" => {
                let (p, id) = (self.point(n(1)), self.num(n(2)));
                let name = self.fresh("j");
                let ok = self.r().add_junction(name.clone(), p.x, p.y, Some(id));
                self.check(ok, "addJunction");
                Value::Junction(name)
            }
            "ConnRef" => {
                let id = self.num(n(1));
                let name = self.fresh("c");
                let ok = self.r().add_connector_without_endpoints(name.clone(), Some(id));
                self.check(ok, "addConnectorWithoutEndpoints");
                Value::Conn(name)
            }
            "ConnRefEnds" => {
                let (s, d, id) = (self.end(n(1)), self.end(n(2)), self.num(n(3)));
                let name = self.fresh("c");
                let (a, b) = (args(&s), args(&d));
                let ok = self.r().add_connector_with_conn_ends(
                    name.clone(),
                    a.0,
                    a.1,
                    a.2,
                    a.3,
                    a.4,
                    a.5,
                    b.0,
                    b.1,
                    b.2,
                    b.3,
                    b.4,
                    b.5,
                    Some(id),
                );
                self.check(ok, "addConnectorWithConnEnds");
                Value::Conn(name)
            }
            "Pin" | "PinOld" => {
                let s = self.shape(n(0));
                let (cls, x, y) = (self.num(n(1)), self.num(n(2)), self.num(n(3)));
                let (prop, inside, dirs) = if f == "Pin" {
                    (self.boolean(n(4)), self.num(n(5)), self.num(n(6)))
                } else {
                    (true, self.num(n(4)), self.num(n(5)))
                };
                let pin = self.fresh("p");
                let ok = self.r().add_shape_connection_pin(
                    s.clone(),
                    pin.clone(),
                    cls,
                    x,
                    y,
                    Some(prop),
                    Some(inside),
                    Some(dirs),
                );
                self.check(ok, "addShapeConnectionPin");
                Value::Pin(s, pin, false)
            }
            "PinJunction" => {
                let j = self.junction(n(0));
                let (cls, dirs) = (self.num(n(1)), self.num(n(2)));
                let pin = self.fresh("p");
                let ok = self.r().add_junction_connection_pin(j.clone(), pin.clone(), cls, Some(dirs));
                self.check(ok, "addJunctionConnectionPin");
                Value::Pin(j, pin, true)
            }
            "processTransaction" => Value::Bool(self.r().process_transaction()),
            "setTransactionUse" => {
                let b = self.boolean(n(1));
                self.r().set_transaction_use(b);
                Value::Void
            }
            "setRoutingParameter" | "setRoutingPenalty" => {
                let (p, val) = (self.num(n(1)), self.num(n(2)));
                let ok = if f == "setRoutingParameter" {
                    self.r().set_routing_parameter(p, Some(val))
                } else {
                    self.r().set_routing_penalty(p, Some(val))
                };
                self.check(ok, f);
                Value::Void
            }
            "setRoutingOption" => {
                let (o, b) = (self.num(n(1)), self.boolean(n(2)));
                let ok = self.r().set_routing_option(o, b);
                self.check(ok, f);
                Value::Void
            }
            "moveShape" => {
                let (s, dx, dy) = (self.shape(n(1)), self.num(n(2)), self.num(n(3)));
                let ok = self.r().move_shape_by(s, dx, dy);
                self.check(ok, "moveShapeBy");
                Value::Void
            }
            "moveShapePoly" => {
                let (s, p, first) = (self.shape(n(1)), self.poly(n(2)), self.boolean(n(3)));
                let ok = self.r().move_polygon_shape(s, flat(&p), Some(first));
                self.check(ok, "movePolygonShape");
                Value::Void
            }
            "moveJunction" => {
                let (j, dx, dy) = (self.junction(n(1)), self.num(n(2)), self.num(n(3)));
                let ok = self.r().move_junction_by(j, dx, dy);
                self.check(ok, "moveJunctionBy");
                Value::Void
            }
            "moveJunctionPt" => {
                let (j, p) = (self.junction(n(1)), self.point(n(2)));
                let ok = self.r().move_junction(j, p.x, p.y);
                self.check(ok, "moveJunction");
                Value::Void
            }
            "deleteShape" => {
                let s = self.shape(n(1));
                let ok = self.r().remove_shape(s);
                self.check(ok, "removeShape");
                Value::Void
            }
            "deleteConnector" => {
                let c = self.conn(n(1));
                let ok = self.r().remove_connector(c);
                self.check(ok, "removeConnector");
                Value::Void
            }
            "deleteJunction" => {
                let j = self.junction(n(1));
                let ok = self.r().remove_junction(j);
                self.check(ok, "removeJunction");
                Value::Void
            }
            "existsOrthogonalSegmentOverlap" => {
                let b = self.boolean(n(1));
                Value::Bool(self.r().exists_orthogonal_segment_overlap(Some(b)))
            }
            "existsOrthogonalFixedSegmentOverlap" => {
                let b = self.boolean(n(1));
                Value::Bool(self.r().exists_orthogonal_fixed_segment_overlap(Some(b)))
            }
            "existsOrthogonalTouchingPaths" => Value::Bool(self.r().exists_orthogonal_touching_paths()),
            "existsCrossings" => {
                let b = self.boolean(n(1));
                Value::Int(self.r().exists_crossings(Some(b)) as i64)
            }
            "existsInvalidOrthogonalPaths" => Value::Bool(self.r().exists_invalid_orthogonal_paths()),
            "hyperedgeRerouter" => Value::Rerouter,
            "registerHyperedgeForReroutingJunction" => {
                let j = self.junction(n(1));
                let idx = self.r().register_hyperedge_for_rerouting_junction(j);
                Value::Int(idx.expect("registerHyperedgeForReroutingJunction") as i64)
            }
            "registerHyperedgeForReroutingEnds" => {
                let list = match n(1) {
                    Value::EndList(l) => l.clone(),
                    o => self.err(&format!("expected a ConnEndList, got {}", o.kind())),
                };
                let idx = self.r().register_hyperedge_for_rerouting(
                    list.iter().map(|e| e.kind).collect(),
                    list.iter().map(|e| e.object.clone()).collect(),
                    list.iter().map(|_| String::new()).collect(),
                    list.iter().map(|e| e.x).collect(),
                    list.iter().map(|e| e.y).collect(),
                    list.iter().map(|e| e.num).collect(),
                );
                Value::Int(idx.expect("registerHyperedgeForRerouting") as i64)
            }
            "newAndDeletedObjectListsFromHyperedgeImprovement" => {
                Value::NdLists(parse(&self.r().new_and_deleted_object_lists_from_hyperedge_improvement()))
            }
            "setSourceEndpoint" | "setDestEndpoint" => {
                let (c, e) = (self.conn(n(0)), self.end(n(1)));
                let a = args(&e);
                let ok = if f == "setSourceEndpoint" {
                    self.r().set_connector_source_conn_end(c, a.0, a.1, a.2, a.3, a.4, a.5)
                } else {
                    self.r().set_connector_dest_conn_end(c, a.0, a.1, a.2, a.3, a.4, a.5)
                };
                self.check(ok, f);
                Value::Void
            }
            "setEndpoints" => {
                let (c, s, d) = (self.conn(n(0)), self.end(n(1)), self.end(n(2)));
                let (a, b) = (args(&s), args(&d));
                let ok =
                    self.r().set_connector_conn_ends(c, a.0, a.1, a.2, a.3, a.4, a.5, b.0, b.1, b.2, b.3, b.4, b.5);
                self.check(ok, f);
                Value::Void
            }
            "setRoutingType" => {
                let (c, t) = (self.conn(n(0)), self.num(n(1)));
                let ok = self.r().set_connector_routing_type(c, t);
                self.check(ok, f);
                Value::Void
            }
            "setRoutingCheckpoints" => {
                let c = self.conn(n(0));
                let cps = match n(1) {
                    Value::Checkpoints(v) => v.clone(),
                    o => self.err(&format!("expected checkpoints, got {}", o.kind())),
                };
                let flat: Vec<f64> = cps.iter().flat_map(|c| [c.0, c.1, c.2 as f64, c.3 as f64]).collect();
                let ok = self.r().set_connector_routing_checkpoints(c, flat);
                self.check(ok, f);
                Value::Void
            }
            "setFixedRoute" => {
                let (c, p) = (self.conn(n(0)), self.poly(n(1)));
                let ok = self.r().set_connector_fixed_route(c, flat(&p));
                self.check(ok, f);
                Value::Void
            }
            "clearFixedRoute" => {
                let c = self.conn(n(0));
                let ok = self.r().clear_connector_fixed_route(c);
                self.check(ok, f);
                Value::Void
            }
            "makePathInvalid" => {
                let c = self.conn(n(0));
                let ok = self.r().make_connector_path_invalid(c);
                self.check(ok, f);
                Value::Void
            }
            "displayRoute" | "route" => {
                let c = self.conn(n(0));
                let text = if f == "route" { self.r().route_connector(c) } else { self.r().display_connector_route(c) };
                let mut p = Polygon::new();
                p.ps = parse(&text).points().into_iter().map(|(x, y)| Point::new(x, y)).collect();
                Value::Poly(p)
            }
            "splitAtSegment" => {
                let (c, seg) = (self.conn(n(0)), self.num(n(1)));
                let (jn, cn) = (self.fresh("j"), self.fresh("c"));
                let r = parse(&self.r().split_connector_at_segment(c, seg, jn, cn));
                if r.is_null() {
                    Value::JcPair(None, None)
                } else {
                    Value::JcPair(Some(r.get("junctionId").str().into()), Some(r.get("connectorId").str().into()))
                }
            }
            "endpointConnEnds" => {
                let c = self.conn(n(0));
                let j = parse(&self.r().connector_endpoint_conn_ends(c));
                Value::EndPair(j.get("source").clone(), j.get("destination").clone())
            }
            "id" => {
                let name = match n(0) {
                    Value::Conn(c) | Value::Shape(c) | Value::Junction(c) => c.clone(),
                    o => self.err(&format!("id() of {}", o.kind())),
                };
                Value::Int(self.r().object_numeric_id(name).expect("objectNumericId") as i64)
            }
            "transformConnectionPinPositions" => {
                let (s, t) = (self.shape(n(0)), self.num(n(1)));
                let ok = self.r().transform_shape_connection_pin_positions(s, t);
                self.check(ok, f);
                Value::Void
            }
            "setPositionFixed" => {
                let (j, b) = (self.junction(n(0)), self.boolean(n(1)));
                let ok = self.r().set_junction_position_fixed(j, b);
                self.check(ok, f);
                Value::Void
            }
            "recommendedPosition" | "position" => {
                let j = self.junction(n(0));
                let text = if f == "position" {
                    self.r().junction_position(j)
                } else {
                    self.r().junction_recommended_position(j)
                };
                let p = parse(&text);
                Value::Point(Point::new(p.get("x").num(), p.get("y").num()))
            }
            "attachedConnectors" => {
                let j = self.junction(n(0));
                Value::ConnList(parse(&self.r().junction_attached_connectors(j)).strings())
            }
            "removeJunctionAndMergeConnectors" => {
                let j = self.junction(n(0));
                match parse(&self.r().remove_junction_and_merge_connectors(j)) {
                    J::Str(c) => Value::Conn(c),
                    _ => Value::Null,
                }
            }
            "setExclusive" => {
                let (owner, pin, junction) = match n(0) {
                    Value::Pin(o, p, j) => (o.clone(), p.clone(), *j),
                    o => self.err(&format!("setExclusive on {}", o.kind())),
                };
                let b = self.boolean(n(1));
                let ok = if junction {
                    self.r().set_junction_connection_pin_exclusive(owner, pin, b)
                } else {
                    self.r().set_shape_connection_pin_exclusive(owner, pin, b)
                };
                self.check(ok, f);
                Value::Void
            }
            "shape" | "junction" => {
                let e = match n(0) {
                    Value::End(Some(e)) => e.clone(),
                    Value::End(None) => return Value::Null,
                    o => self.err(&format!("{} of {}", f, o.kind())),
                };
                match (f, e.kind as u32) {
                    ("shape", 1) => Value::Shape(e.object),
                    ("junction", 2) => Value::Junction(e.object),
                    _ => Value::Null,
                }
            }
            "size" => Value::Int(match n(0) {
                Value::Poly(p) => p.ps.len(),
                Value::ConnList(l) | Value::JunctionList(l) => l.len(),
                Value::Checkpoints(l) => l.len(),
                o => self.err(&format!("size() of {}", o.kind())),
            } as i64),
            "ps" => {
                let (p, i) = (self.poly(n(0)), self.int(n(1)));
                Value::Point(p.ps[i as usize])
            }
            "at" => {
                let i = self.int(n(1));
                match n(0) {
                    Value::Checkpoints(v) => Value::Checkpoint(v[i as usize]),
                    o => self.err(&format!("[{}] on {}", i, o.kind())),
                }
            }
            "x" | "y" => {
                let p = self.point(n(0));
                Value::Double(if f == "x" { p.x } else { p.y })
            }
            "_id" => Value::Int(self.poly(n(0))._id as i64),
            "first" | "second" => {
                let first = f == "first";
                match n(0) {
                    Value::JcPair(j, c) => {
                        if first {
                            j.clone().map(Value::Junction).unwrap_or(Value::Null)
                        } else {
                            c.clone().map(Value::Conn).unwrap_or(Value::Null)
                        }
                    }
                    Value::EndPair(a, b) => self.json_end(if first { a } else { b }),
                    o => self.err(&format!("{} of {}", f, o.kind())),
                }
            }
            "newJunctionList" | "deletedJunctionList" => match n(0) {
                Value::NdLists(l) => Value::JunctionList(
                    l.get(if f == "newJunctionList" { "newJunctionIds" } else { "deletedJunctionIds" }).strings(),
                ),
                o => self.err(&format!("{} of {}", f, o.kind())),
            },
            "newConnectorList" | "deletedConnectorList" | "changedConnectorList" => match n(0) {
                Value::NdLists(l) => Value::ConnList(
                    l.get(match f {
                        "newConnectorList" => "newConnectorIds",
                        "deletedConnectorList" => "deletedConnectorIds",
                        _ => "changedConnectorIds",
                    })
                    .strings(),
                ),
                o => self.err(&format!("{} of {}", f, o.kind())),
            },
            "begin" | "end" => match n(0) {
                Value::JunctionList(l) => Value::JIter(l.clone(), if f == "begin" { 0 } else { l.len() }),
                Value::ConnList(l) => Value::CIter(l.clone(), if f == "begin" { 0 } else { l.len() }),
                o => self.err(&format!("{}() of {}", f, o.kind())),
            },
            "deref" => match n(0) {
                Value::JIter(l, i) if *i < l.len() => Value::Junction(l[*i].clone()),
                Value::CIter(l, i) if *i < l.len() => Value::Conn(l[*i].clone()),
                o => self.err(&format!("bad dereference of {}", o.kind())),
            },
            "next" => match n(0) {
                Value::JIter(l, i) if *i < l.len() => Value::JIter(l.clone(), i + 1),
                Value::CIter(l, i) if *i < l.len() => Value::CIter(l.clone(), i + 1),
                o => self.err(&format!("bad increment of {}", o.kind())),
            },
            _ => self.err(&format!("unknown function `{}`", f)),
        }
    }

    fn binop(&self, op: &str, a: &Value, b: &Value) -> Value {
        let ptr_eq = match (a, b) {
            (Value::Null, Value::Null) => Some(true),
            (Value::Null, x) | (x, Value::Null) if POINTER_TYPES.contains(&x.kind()) => Some(false),
            (Value::Conn(x), Value::Conn(y))
            | (Value::Shape(x), Value::Shape(y))
            | (Value::Junction(x), Value::Junction(y)) => Some(x == y),
            (Value::Pin(a1, b1, _), Value::Pin(a2, b2, _)) => Some(a1 == a2 && b1 == b2),
            (Value::JIter(l1, i1), Value::JIter(l2, i2)) | (Value::CIter(l1, i1), Value::CIter(l2, i2)) => {
                Some(l1 == l2 && i1 == i2)
            }
            _ => None,
        };
        if let Some(eq) = ptr_eq {
            return match op {
                "==" => Value::Bool(eq),
                "!=" => Value::Bool(!eq),
                _ => self.err(&format!("operator {} on pointers", op)),
            };
        }
        let both_int = !matches!(a, Value::Double(_)) && !matches!(b, Value::Double(_));
        if both_int {
            let (x, y) = (self.int(a), self.int(b));
            return match op {
                "+" => Value::Int(x + y),
                "-" => Value::Int(x - y),
                "*" => Value::Int(x * y),
                "/" => Value::Int(x / y),
                "%" => Value::Int(x % y),
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
            Expr::Call(f, a) => {
                let name = match a.first() {
                    Some(Expr::Atom(n)) => n.clone(),
                    _ => self.err("unsupported assignment target"),
                };
                let idx = if a.len() > 1 {
                    let iv = self.eval(&a[1]);
                    Some(self.int(&iv))
                } else {
                    None
                };
                let rhs_num = match &rhs {
                    Value::Int(_) | Value::Double(_) | Value::Bool(_) => Some(self.num(&rhs)),
                    _ => None,
                };
                let target = self.var_mut(&name);
                match (f.as_str(), target, rhs) {
                    ("ps", Value::Poly(p), Value::Point(pt)) => p.ps[idx.unwrap() as usize] = pt,
                    ("at", Value::Checkpoints(v), Value::Checkpoint(c)) => v[idx.unwrap() as usize] = c,
                    ("x", Value::Point(p), _) if rhs_num.is_some() => p.x = rhs_num.unwrap(),
                    ("y", Value::Point(p), _) if rhs_num.is_some() => p.y = rhs_num.unwrap(),
                    ("_id", Value::Poly(p), Value::Int(i)) => p._id = i as i32,
                    _ => panic!("fixture-ops interpreter: unsupported assignment"),
                }
            }
            Expr::Str(_) => self.err("cannot assign to a string"),
        }
    }

    /// Upstream fixtures print from connector callbacks; the wrapper
    /// reports the connectors whose callback fired as
    /// `lastTransactionUpdatedConnectorIds()`.
    fn flush_callbacks(&mut self) {
        let ids = parse(&self.r().last_transaction_updated_connector_ids()).strings();
        for id in ids {
            let Some(kind) = self.callbacks.get(&id).copied() else {
                continue;
            };
            let nid = self.r().object_numeric_id(id.clone()).unwrap();
            self.stdout.push_str(&format!("Connector {} needs rerouting!\n", nid));
            let text = match kind {
                CbKind::Route => self.r().route_connector(id.clone()),
                CbKind::DisplayRoute => self.r().display_connector_route(id.clone()),
            };
            let route = parse(&text).points();
            self.stdout.push_str("New path: ");
            for (i, (x, y)) in route.iter().enumerate() {
                self.stdout.push_str(&format!(
                    "{}({}, {})",
                    if i > 0 { "-" } else { "" },
                    printf_f(*x),
                    printf_f(*y)
                ));
            }
            self.stdout.push('\n');
        }
    }

    fn exec_block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.line = s.line;
            let before = self.router.as_ref().map(|r| r.transaction_count());
            let r = panic::catch_unwind(AssertUnwindSafe(|| self.exec(s)));
            if let Err(payload) = r {
                eprintln!("wrapper-ops: panic while executing {}.ops:{}: `{}`", self.name, s.line, s.text);
                panic::resume_unwind(payload);
            }
            if !matches!(s.op, Op::If(..)) && !self.router_deleted {
                let after = self.router.as_ref().map(|r| r.transaction_count());
                if before.is_some() && before != after {
                    self.flush_callbacks();
                }
            }
        }
    }

    /// Compares the dump and every connector's display route JSON with the
    /// golden.
    fn compare(&mut self, golden: &str) {
        let text = self.r().output_diagram_text(Some(true));
        if let Err(msg) = compare_with_golden_in(&text, GoldenDir::Bump, golden) {
            panic!("wrapper golden-bump {}: {}", golden, msg);
        }
        // Routes through the JSON API: golden `path` blocks by numeric id.
        let golden_text = read_golden_file_in(GoldenDir::Bump, golden, "txt");
        let conns = parse(&self.r().connector_ids()).strings();
        let mut by_numeric: HashMap<u32, Vec<String>> = HashMap::new();
        for c in conns {
            let nid = self.r().object_numeric_id(c.clone()).unwrap();
            by_numeric.entry(nid).or_default().push(c);
        }
        let mut lines = golden_text.lines().peekable();
        while let Some(l) = lines.next() {
            if l != "path" {
                continue;
            }
            let id: u32 = lines.next().unwrap().trim_start_matches("id=").parse().unwrap();
            let mut pts = Vec::new();
            while let Some(p) = lines.peek() {
                if !p.starts_with('p') {
                    break;
                }
                let toks: Vec<&str> = p.split_whitespace().collect();
                pts.push((toks[1].parse::<f64>().unwrap(), toks[2].parse::<f64>().unwrap()));
                lines.next();
            }
            let candidates = by_numeric.get(&id).cloned().unwrap_or_default();
            let matched = candidates.iter().any(|c| {
                let route = parse(&self.r().display_connector_route(c.clone())).points();
                route.len() == pts.len()
                    && route
                        .iter()
                        .zip(pts.iter())
                        .all(|(a, b)| (a.0 - b.0).abs() <= 1e-9 && (a.1 - b.1).abs() <= 1e-9)
            });
            assert!(matched, "wrapper golden {}: displayConnectorRoute of connector {} differs", golden, id);
        }
        self.outputs_checked += 1;
    }

    fn exec(&mut self, s: &Stmt) {
        match &s.op {
            Op::Let(name, ty, e) => {
                let v = self.eval(e);
                if ty == "router" {
                    self.router_var = Some(name.clone());
                    return;
                }
                let v = self.coerce(ty, v);
                self.vars.insert(name.clone(), (ty.clone(), v));
            }
            Op::Decl(name, ty) => {
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
            Op::Output { golden, checked, .. } => {
                if self.golden_mode && *checked {
                    self.compare(golden);
                } else {
                    let _ = self.r().output_diagram_text(Some(true));
                }
            }
            Op::Delete { golden, .. } => {
                match golden {
                    Some(g) if self.golden_mode => self.compare(g),
                    _ => {
                        let _ = self.r().output_diagram_text(Some(true));
                    }
                }
                self.router_deleted = true;
            }
            Op::Callback(conn, kind) => {
                let c = match self.var(conn) {
                    Value::Conn(c) => c,
                    o => self.err(&format!("callback on {}", o.kind())),
                };
                self.callbacks.insert(c, *kind);
            }
            Op::Print(text) => self.stdout.push_str(text),
            Op::Assert(e) => {
                let v = self.eval(e);
                assert!(self.boolean(&v), "fixture {} (wrapper): assertion failed: {}", self.name, s.text);
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
}

/// Result of a wrapper run.
pub struct WrapperRun {
    pub exit_code: i64,
    pub stdout: String,
    pub golden_mode: bool,
    pub outputs_checked: usize,
}

/// Runs `tests/fixtures/<name>.ops` through the wrapper; panics on any
/// mismatch with the goldens.  `check_stdout`: compare the printed text.
pub fn run_fixture_via_wrapper(name: &str, check_stdout: bool) -> WrapperRun {
    let path = format!("{}/tests/fixtures/{}.ops", env!("CARGO_MANIFEST_DIR"), name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let script = parse_script(name, &text);
    let mut it = Interp {
        name: name.to_string(),
        router: None,
        router_var: None,
        router_deleted: false,
        vars: HashMap::new(),
        stdout: String::new(),
        callbacks: HashMap::new(),
        ret: None,
        line: 0,
        golden_mode: script.golden_mode,
        counter: 0,
        outputs_checked: 0,
    };
    it.exec_block(&script.body);
    let exit_code = it.ret.expect("script ended without ret");
    if script.golden_mode {
        assert_eq!(
            (exit_code & 0xff) as i32,
            golden_exit_code(&script.fixture),
            "fixture {} (wrapper): exit code",
            name
        );
        if script.prints && check_stdout {
            let golden = read_golden_file(&script.fixture, "stdout");
            assert_eq!(it.stdout, golden, "fixture {} (wrapper): stdout differs from upstream", name);
        }
    }
    WrapperRun { exit_code, stdout: it.stdout, golden_mode: script.golden_mode, outputs_checked: it.outputs_checked }
}
