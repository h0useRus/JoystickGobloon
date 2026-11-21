//! Lua data files for plugins: a table constructor (`local diff = { ["key"] = "JOY_BTN1", ... }`)
//! read without running anything, and written back with sorted keys and tab indentation, the way
//! games that keep their settings in Lua tables write them.

use mlua::{Lua, Table, Value};

/// A value of a Lua data file.
#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    Nil,
    Bool(bool),
    Number(f64),
    Str(String),
    /// Entries in the order written; positional values get the keys 1, 2, ...
    Table(Vec<(Data, Data)>),
}

/// Most nested tables: a broken file cannot overflow the stack.
const MAX_DEPTH: usize = 200;

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
    depth: usize,
}

/// The first table constructor in `text` (after `local name =` or `return`); text after it
/// is ignored.
pub fn parse(text: &str) -> Result<Data, String> {
    let mut p = Parser {
        src: text.as_bytes(),
        at: 0,
        depth: 0,
    };
    loop {
        p.skip_space()?;
        match p.peek() {
            None => return Err("no table in the file".into()),
            Some(b'{') => return p.table(),
            Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'=' => p.at += 1,
            Some(c) => return Err(p.error(&format!("unexpected '{}'", c as char))),
        }
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    fn error(&self, what: &str) -> String {
        let line = self.src[..self.at.min(self.src.len())]
            .iter()
            .filter(|&&c| c == b'\n')
            .count()
            + 1;
        format!("line {line}: {what}")
    }

    fn skip_space(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => self.at += 1,
                Some(b'-') if self.src.get(self.at + 1) == Some(&b'-') => {
                    self.at += 2;
                    if let Some(level) = self.long_bracket() {
                        self.long_string(level)?;
                    } else {
                        while self.peek().is_some_and(|c| c != b'\n') {
                            self.at += 1;
                        }
                    }
                }
                Some(0xEF) if self.src[self.at..].starts_with(b"\xEF\xBB\xBF") => self.at += 3,
                _ => return Ok(()),
            }
        }
    }

    /// At `[[` or `[==[`: its level, and moves past it.
    fn long_bracket(&mut self) -> Option<usize> {
        if self.peek() != Some(b'[') {
            return None;
        }
        let mut i = self.at + 1;
        while self.src.get(i) == Some(&b'=') {
            i += 1;
        }
        if self.src.get(i) == Some(&b'[') {
            let level = i - self.at - 1;
            self.at = i + 1;
            Some(level)
        } else {
            None
        }
    }

    fn long_string(&mut self, level: usize) -> Result<String, String> {
        let close = format!("]{}]", "=".repeat(level));
        let rest = &self.src[self.at..];
        let Some(end) = rest
            .windows(close.len())
            .position(|w| w == close.as_bytes())
        else {
            return Err(self.error("unfinished long string"));
        };
        let mut s = &rest[..end];
        if s.first() == Some(&b'\n') {
            s = &s[1..];
        }
        self.at += end + close.len();
        Ok(String::from_utf8_lossy(s).into_owned())
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.skip_space()?;
        if self.peek() == Some(c) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(&format!("'{}' expected", c as char)))
        }
    }

    fn table(&mut self) -> Result<Data, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("tables nested too deep"));
        }
        self.expect(b'{')?;
        let mut entries = Vec::new();
        let mut next = 1.0;
        loop {
            self.skip_space()?;
            match self.peek() {
                Some(b'}') => {
                    self.at += 1;
                    break;
                }
                None => return Err(self.error("unfinished table")),
                _ => {}
            }
            let save = self.at;
            if self.peek() == Some(b'[') && self.long_bracket().is_none() {
                // [key] = value
                self.at += 1;
                let key = self.value()?;
                self.expect(b']')?;
                self.expect(b'=')?;
                let value = self.value()?;
                entries.push((key, value));
            } else {
                self.at = save;
                // name = value, or a positional value.
                let name = self.name();
                self.skip_space()?;
                if let Some(name) = name.filter(|_| {
                    self.peek() == Some(b'=') && self.src.get(self.at + 1) != Some(&b'=')
                }) {
                    self.at += 1;
                    let value = self.value()?;
                    entries.push((Data::Str(name), value));
                } else {
                    self.at = save;
                    let value = self.value()?;
                    entries.push((Data::Number(next), value));
                    next += 1.0;
                }
            }
            self.skip_space()?;
            match self.peek() {
                Some(b',' | b';') => self.at += 1,
                Some(b'}') => {}
                _ => return Err(self.error("',' or '}' expected")),
            }
        }
        self.depth -= 1;
        Ok(Data::Table(entries))
    }

    fn name(&mut self) -> Option<String> {
        let start = self.at;
        if !self
            .peek()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        {
            return None;
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.at += 1;
        }
        Some(String::from_utf8_lossy(&self.src[start..self.at]).into_owned())
    }

    fn value(&mut self) -> Result<Data, String> {
        self.skip_space()?;
        match self.peek() {
            Some(b'{') => self.table(),
            Some(q @ (b'"' | b'\'')) => self.string(q).map(Data::Str),
            Some(b'[') => match self.long_bracket() {
                Some(level) => self.long_string(level).map(Data::Str),
                None => Err(self.error("value expected")),
            },
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'.' => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let name = self.name().unwrap_or_default();
                match name.as_str() {
                    "true" => Ok(Data::Bool(true)),
                    "false" => Ok(Data::Bool(false)),
                    "nil" => Ok(Data::Nil),
                    _ => Err(self.error(&format!("'{name}' is not data"))),
                }
            }
            _ => Err(self.error("value expected")),
        }
    }

    fn number(&mut self) -> Result<Data, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
            self.skip_space()?;
        }
        let digits = self.at;
        while self.peek().is_some_and(|c| {
            c.is_ascii_alphanumeric()
                || c == b'.'
                || ((c == b'-' || c == b'+')
                    && matches!(self.src.get(self.at - 1), Some(b'e' | b'E')))
        }) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.src[digits..self.at]).unwrap_or_default();
        let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(hex) => i64::from_str_radix(hex, 16).ok().map(|v| v as f64),
            None => text.parse::<f64>().ok(),
        };
        let Some(value) = value else {
            self.at = start;
            return Err(self.error(&format!("bad number '{text}'")));
        };
        let negative = self.src[start] == b'-';
        Ok(Data::Number(if negative { -value } else { value }))
    }

    fn string(&mut self, quote: u8) -> Result<String, String> {
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.error("unfinished string"));
            };
            self.at += 1;
            match c {
                b'\n' => return Err(self.error("unfinished string")),
                c if c == quote => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.error("unfinished string"));
                    };
                    self.at += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'a' => out.push(7),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'v' => out.push(11),
                        b'\n' => out.push(b'\n'),
                        b'0'..=b'9' => {
                            let mut n = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'9') => {
                                        n = n * 10 + u32::from(d - b'0');
                                        self.at += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(n.min(255) as u8);
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

/// `value` as Lua data text: tables over several lines with tabs, `[key] = value,` entries with
/// number keys first, then strings, each sorted.
pub fn write(value: &Data) -> String {
    let mut out = String::new();
    write_value(value, 0, &mut out);
    out
}

fn write_value(value: &Data, depth: usize, out: &mut String) {
    match value {
        Data::Nil => out.push_str("nil"),
        Data::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Data::Number(n) => out.push_str(&number(*n)),
        Data::Str(s) => quote(s, out),
        Data::Table(entries) => {
            let mut sorted: Vec<&(Data, Data)> = entries
                .iter()
                .filter(|(k, v)| !matches!(k, Data::Nil) && !matches!(v, Data::Nil))
                .collect();
            sorted.sort_by(|(a, _), (b, _)| key_order(a, b));
            out.push('{');
            out.push('\n');
            for (k, v) in sorted {
                push_tabs(depth + 1, out);
                out.push('[');
                write_value(k, depth + 1, out);
                out.push_str("] = ");
                write_value(v, depth + 1, out);
                out.push_str(",\n");
            }
            push_tabs(depth, out);
            out.push('}');
        }
    }
}

fn push_tabs(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push('\t');
    }
}

fn key_order(a: &Data, b: &Data) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Data::Number(x), Data::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Data::Number(_), _) => Ordering::Less,
        (_, Data::Number(_)) => Ordering::Greater,
        (Data::Str(x), Data::Str(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 => out.push_str(&format!("\\{}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Data as a Lua value: tables become plain tables.
pub fn to_lua(lua: &Lua, data: &Data) -> mlua::Result<Value> {
    Ok(match data {
        Data::Nil => Value::Nil,
        Data::Bool(b) => Value::Boolean(*b),
        Data::Number(n) => Value::Number(*n),
        Data::Str(s) => Value::String(lua.create_string(s)?),
        Data::Table(entries) => {
            let t = lua.create_table()?;
            for (k, v) in entries {
                if matches!(k, Data::Nil) {
                    continue;
                }
                t.raw_set(to_lua(lua, k)?, to_lua(lua, v)?)?;
            }
            Value::Table(t)
        }
    })
}

/// A Lua value as data; functions and the like are errors, a table seen twice too.
pub fn from_lua(value: &Value) -> mlua::Result<Data> {
    from_lua_at(value, 0)
}

fn from_lua_at(value: &Value, depth: usize) -> mlua::Result<Data> {
    Ok(match value {
        Value::Nil => Data::Nil,
        Value::Boolean(b) => Data::Bool(*b),
        Value::Integer(i) => Data::Number(*i as f64),
        Value::Number(n) => Data::Number(*n),
        Value::String(s) => Data::Str(s.to_string_lossy()),
        Value::Table(t) => {
            if depth > MAX_DEPTH {
                return Err(mlua::Error::runtime(
                    "luadata.write: tables nested too deep (or a table inside itself)",
                ));
            }
            table_data(t, depth)?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "luadata.write: a {} is not data",
                other.type_name()
            )));
        }
    })
}

fn table_data(t: &Table, depth: usize) -> mlua::Result<Data> {
    let mut entries = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair?;
        entries.push((from_lua_at(&k, depth + 1)?, from_lua_at(&v, depth + 1)?));
    }
    Ok(Data::Table(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "local diff = {\n\t[\"axisDiffs\"] = {\n\t\t[\"a2001cdnil\"] = {\n\t\t\t[\"added\"] = {\n\t\t\t\t[1] = {\n\t\t\t\t\t[\"filter\"] = {\n\t\t\t\t\t\t[\"curvature\"] = {\n\t\t\t\t\t\t\t[1] = 0.15,\n\t\t\t\t\t\t},\n\t\t\t\t\t\t[\"deadzone\"] = 0,\n\t\t\t\t\t\t[\"invert\"] = true,\n\t\t\t\t\t},\n\t\t\t\t\t[\"key\"] = \"JOY_Y\",\n\t\t\t\t},\n\t\t\t},\n\t\t\t[\"name\"] = \"Pitch \\\"stick\\\"\",\n\t\t},\n\t},\n}\nreturn diff\n";

    #[test]
    fn round_trips_a_file_written_sorted() {
        let data = parse(DIFF).unwrap();
        let text = format!("local diff = {}\nreturn diff\n", write(&data));
        assert_eq!(text, DIFF);
    }

    #[test]
    fn reads_every_form_of_entry() {
        let data = parse(
            "-- comment\nreturn { a = 1, 'two', [3] = -0x10, --[[ long\ncomment ]] b = { true, nil, false }; [\"c\"] = [[long\nstring]], d = 1e3 }",
        )
        .unwrap();
        let Data::Table(e) = data else { panic!() };
        assert_eq!(e[0], (Data::Str("a".into()), Data::Number(1.0)));
        assert_eq!(e[1], (Data::Number(1.0), Data::Str("two".into())));
        assert_eq!(e[2], (Data::Number(3.0), Data::Number(-16.0)));
        assert_eq!(
            e[3].1,
            Data::Table(vec![
                (Data::Number(1.0), Data::Bool(true)),
                (Data::Number(2.0), Data::Nil),
                (Data::Number(3.0), Data::Bool(false)),
            ])
        );
        assert_eq!(e[4].1, Data::Str("long\nstring".into()));
        assert_eq!(e[5].1, Data::Number(1000.0));
    }

    #[test]
    fn refuses_code() {
        assert!(parse("local x = { a = os.exit() }").is_err());
        assert!(parse("local x = { a = 1").is_err());
        assert!(parse("nothing here").is_err());
    }

    #[test]
    fn writes_numbers_and_strings_plainly() {
        let t = Data::Table(vec![
            (Data::Str("b".into()), Data::Number(0.5)),
            (Data::Str("a".into()), Data::Str("x\ny".into())),
            (Data::Number(2.0), Data::Number(-3.0)),
            (Data::Number(1.0), Data::Table(Vec::new())),
        ]);
        assert_eq!(
            write(&t),
            "{\n\t[1] = {\n\t},\n\t[2] = -3,\n\t[\"a\"] = \"x\\ny\",\n\t[\"b\"] = 0.5,\n}"
        );
    }
}
