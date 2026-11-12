//! Tiny JSON parser for the wrapper tests (the crate has no serde).

#![allow(dead_code)]

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or(&J::Null),
            _ => &J::Null,
        }
    }
    pub fn has(&self, key: &str) -> bool {
        matches!(self, J::Obj(f) if f.iter().any(|(k, _)| k == key))
    }
    pub fn keys(&self) -> Vec<String> {
        match self {
            J::Obj(f) => f.iter().map(|(k, _)| k.clone()).collect(),
            _ => panic!("not an object: {:?}", self),
        }
    }
    pub fn num(&self) -> f64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("not a number: {:?}", self),
        }
    }
    pub fn boolean(&self) -> bool {
        match self {
            J::Bool(b) => *b,
            _ => panic!("not a bool: {:?}", self),
        }
    }
    pub fn str(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => panic!("not a string: {:?}", self),
        }
    }
    pub fn opt_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn arr(&self) -> &Vec<J> {
        match self {
            J::Arr(a) => a,
            _ => panic!("not an array: {:?}", self),
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
    pub fn strings(&self) -> Vec<String> {
        self.arr().iter().map(|s| s.str().to_string()).collect()
    }
    pub fn points(&self) -> Vec<(f64, f64)> {
        self.arr().iter().map(|p| (p.get("x").num(), p.get("y").num())).collect()
    }
}

pub fn parse(text: &str) -> J {
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    let v = value(&b, &mut i);
    ws(&b, &mut i);
    assert_eq!(i, b.len(), "trailing JSON: {}", text);
    v
}

fn ws(b: &[char], i: &mut usize) {
    while *i < b.len() && b[*i].is_whitespace() {
        *i += 1;
    }
}

fn value(b: &[char], i: &mut usize) -> J {
    ws(b, i);
    match b[*i] {
        'n' => {
            *i += 4;
            J::Null
        }
        't' => {
            *i += 4;
            J::Bool(true)
        }
        'f' => {
            *i += 5;
            J::Bool(false)
        }
        '"' => J::Str(string(b, i)),
        '[' => {
            *i += 1;
            let mut out = Vec::new();
            ws(b, i);
            if b[*i] == ']' {
                *i += 1;
                return J::Arr(out);
            }
            loop {
                out.push(value(b, i));
                ws(b, i);
                let c = b[*i];
                *i += 1;
                if c == ']' {
                    return J::Arr(out);
                }
                assert_eq!(c, ',');
            }
        }
        '{' => {
            *i += 1;
            let mut out = Vec::new();
            ws(b, i);
            if b[*i] == '}' {
                *i += 1;
                return J::Obj(out);
            }
            loop {
                ws(b, i);
                let k = string(b, i);
                ws(b, i);
                assert_eq!(b[*i], ':');
                *i += 1;
                out.push((k, value(b, i)));
                ws(b, i);
                let c = b[*i];
                *i += 1;
                if c == '}' {
                    return J::Obj(out);
                }
                assert_eq!(c, ',');
            }
        }
        _ => {
            let start = *i;
            while *i < b.len() && "+-0123456789.eE".contains(b[*i]) {
                *i += 1;
            }
            let s: String = b[start..*i].iter().collect();
            J::Num(s.parse().unwrap_or_else(|_| panic!("bad JSON number {:?}", s)))
        }
    }
}

fn string(b: &[char], i: &mut usize) -> String {
    assert_eq!(b[*i], '"');
    *i += 1;
    let mut out = String::new();
    while b[*i] != '"' {
        if b[*i] == '\\' {
            *i += 1;
            match b[*i] {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let h: String = b[*i + 1..*i + 5].iter().collect();
                    out.push(char::from_u32(u32::from_str_radix(&h, 16).unwrap()).unwrap());
                    *i += 4;
                }
                c => out.push(c),
            }
        } else {
            out.push(b[*i]);
        }
        *i += 1;
    }
    *i += 1;
    out
}
