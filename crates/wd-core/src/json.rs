//! Minimal JSON writer.
pub fn str_lit(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Numbers that are NaN or infinite become `null`, like `JSON.stringify`.
pub fn num(v: f64) -> String {
    if v == 0.0 { "0".to_string() } else if v.is_finite() { format!("{}", v) } else { "null".to_string() }
}

pub fn opt(v: Option<i64>) -> String {
    match v { Some(n) => n.to_string(), None => "null".to_string() }
}

pub struct Obj { buf: String, first: bool }
impl Obj {
    pub fn new() -> Self { Obj { buf: String::from("{"), first: true } }
    pub fn field(&mut self, key: &str, raw_value: String) {
        if !self.first { self.buf.push(','); }
        self.first = false;
        self.buf.push_str(&str_lit(key));
        self.buf.push(':');
        self.buf.push_str(&raw_value);
    }
    pub fn finish(mut self) -> String { self.buf.push('}'); self.buf }
}
