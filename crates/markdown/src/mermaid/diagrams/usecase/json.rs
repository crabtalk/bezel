//! The body of a `json` node: its value, and the order its keys were
//! written in. Pele's `diagrams/usecase/json.ts`, where `JSON.parse` stands
//! in: its errors are worded as V8 words them.

use indexmap::IndexMap;

use crate::mermaid::{
    config::{Value, parse_json},
    detect::Kind,
    error::Error,
    util::chars::{NONE, is_digit, is_hex},
};

/// Characters of context either side of an unexpected token, as V8 shows
/// them, and the longest text V8 shows whole.
const CONTEXT: usize = 10;
const SHORT: usize = 2 * CONTEXT + 1;
/// The deepest nesting read, as [`parse_json`] reads it.
const MAX_DEPTH: usize = 512;

pub struct OrderedJson {
    pub value: Value,
    pub property_order: IndexMap<String, Vec<String>>,
}

/// JSON text as UTF-16 units.
struct Units<'a>(&'a [u16]);

impl Units<'_> {
    fn code(&self, at: usize) -> u32 {
        self.0.get(at).map_or(NONE, |&u| u32::from(u))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn slice(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.0[from.min(to)..to.min(self.0.len())])
    }
}

struct Frame {
    object: bool,
    pointer: String,
    /// For each key read so far, the stretch of `created` its value added.
    ranges: IndexMap<String, (usize, usize)>,
    key: String,
    from: usize,
    index: usize,
}

#[derive(PartialEq)]
enum State {
    Value,
    Key,
    After,
}

fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Walks JSON text that `JSON.parse` has accepted, to recover the order its
/// keys were written in, which `JSON.parse` loses for integer-like keys. On
/// text `JSON.parse` rejected it finds where, as the offset of the first
/// character that is not valid JSON. Nesting is kept on a stack of its own,
/// so depth is limited by memory and not by the call stack.
fn property_order(text: &Units) -> Result<IndexMap<String, Vec<String>>, usize> {
    let mut order: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut created: Vec<String> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let n = text.len();
    let mut at = 0;
    let mut pointer = String::new();
    let mut state = State::Value;

    let skip = |at: &mut usize| {
        while *at < n {
            let c = text.code(*at);
            if c != 32 && c != 9 && c != 10 && c != 13 {
                return;
            }
            *at += 1;
        }
    };

    let read_string = |at: &mut usize, decode: bool| -> Result<String, usize> {
        *at += 1;
        let mut value: Vec<u16> = Vec::new();
        while *at < n {
            let start = *at;
            let ch = text.code(*at);
            *at += 1;
            if ch == 34 {
                return Ok(String::from_utf16_lossy(&value));
            }
            if ch < 0x20 {
                return Err(start);
            }
            if ch != 92 {
                if decode {
                    value.push(ch as u16);
                }
                continue;
            }
            let escape_at = *at;
            let escape = text.code(*at);
            *at += 1;
            let decoded: u16 = match escape {
                34 | 92 | 47 => escape as u16,
                98 => 8,
                102 => 12,
                110 => 10,
                114 => 13,
                116 => 9,
                117 => {
                    if *at + 4 > n || !(0..4).all(|k| is_hex(text.code(*at + k))) {
                        return Err(*at);
                    }
                    let unit =
                        u16::from_str_radix(&text.slice(*at, *at + 4), 16).map_err(|_| *at)?;
                    *at += 4;
                    unit
                }
                _ => return Err(escape_at),
            };
            if decode {
                value.push(decoded);
            }
        }
        Err(*at)
    };

    let literal = |at: &mut usize, word: &str| -> Result<(), usize> {
        for (k, b) in word.bytes().enumerate() {
            if text.code(*at + k) != u32::from(b) {
                return Err(*at + k);
            }
        }
        *at += word.len();
        Ok(())
    };

    let number = |at: &mut usize| -> Result<(), usize> {
        if text.code(*at) == 45 {
            *at += 1;
        }
        if text.code(*at) == 48 {
            *at += 1;
        } else if is_digit(text.code(*at)) {
            while is_digit(text.code(*at)) {
                *at += 1;
            }
        } else {
            return Err(*at);
        }
        if text.code(*at) == 46 {
            *at += 1;
            if !is_digit(text.code(*at)) {
                return Err(*at);
            }
            while is_digit(text.code(*at)) {
                *at += 1;
            }
        }
        if (text.code(*at) | 32) == 101 {
            *at += 1;
            if text.code(*at) == 43 || text.code(*at) == 45 {
                *at += 1;
            }
            if !is_digit(text.code(*at)) {
                return Err(*at);
            }
            while is_digit(text.code(*at)) {
                *at += 1;
            }
        }
        Ok(())
    };

    skip(&mut at);
    loop {
        if state == State::Value {
            skip(&mut at);
            let c = text.code(at);
            state = State::After;
            if c == 123 {
                at += 1;
                order.insert(pointer.clone(), Vec::new());
                created.push(pointer.clone());
                skip(&mut at);
                if text.code(at) == 125 {
                    at += 1;
                } else {
                    stack.push(Frame {
                        object: true,
                        pointer: pointer.clone(),
                        ranges: IndexMap::new(),
                        key: String::new(),
                        from: 0,
                        index: 0,
                    });
                    state = State::Key;
                }
            } else if c == 91 {
                at += 1;
                skip(&mut at);
                if text.code(at) == 93 {
                    at += 1;
                } else {
                    stack.push(Frame {
                        object: false,
                        pointer: pointer.clone(),
                        ranges: IndexMap::new(),
                        key: String::new(),
                        from: 0,
                        index: 0,
                    });
                    pointer.push_str("/0");
                    state = State::Value;
                }
            } else if c == 34 {
                read_string(&mut at, false)?;
            } else if c == 116 {
                literal(&mut at, "true")?;
            } else if c == 102 {
                literal(&mut at, "false")?;
            } else if c == 110 {
                literal(&mut at, "null")?;
            } else if c == 45 || is_digit(c) {
                number(&mut at)?;
            } else {
                return Err(at);
            }
        } else if state == State::Key {
            if text.code(at) != 34 {
                return Err(at);
            }
            let key = read_string(&mut at, true)?;
            let Some(frame) = stack.last_mut() else {
                return Err(at);
            };
            // A repeated key keeps its first place in the order and takes
            // the nested order of its last value.
            match frame.ranges.get(&key) {
                None => {
                    if let Some(keys) = order.get_mut(&frame.pointer) {
                        keys.push(key.clone());
                    }
                }
                Some(&(from, to)) => {
                    for made in &created[from..to] {
                        order.shift_remove(made);
                    }
                }
            }
            frame.key = key.clone();
            frame.from = created.len();
            skip(&mut at);
            if text.code(at) != 58 {
                return Err(at);
            }
            at += 1;
            pointer = format!("{}/{}", frame.pointer, pointer_key(&key));
            state = State::Value;
        } else {
            let Some(frame) = stack.last_mut() else {
                break;
            };
            if frame.object {
                frame
                    .ranges
                    .insert(frame.key.clone(), (frame.from, created.len()));
            } else {
                frame.index += 1;
            }
            skip(&mut at);
            let c = text.code(at);
            if c == if frame.object { 125 } else { 93 } {
                at += 1;
                stack.pop();
            } else if c != 44 {
                return Err(at);
            } else {
                at += 1;
                skip(&mut at);
                pointer = format!("{}/{}", frame.pointer, frame.index);
                state = if frame.object {
                    State::Key
                } else {
                    State::Value
                };
            }
        }
    }
    skip(&mut at);
    if at != n {
        return Err(at);
    }
    Ok(order)
}

/// Why V8's `JSON.parse` rejects `text`, in its words.
struct V8<'a> {
    text: &'a Units<'a>,
    at: usize,
}

impl V8<'_> {
    fn code(&self) -> u32 {
        self.text.code(self.at)
    }

    fn skip(&mut self) {
        while matches!(self.code(), 32 | 9 | 10 | 13) {
            self.at += 1;
        }
    }

    fn at_position(&self, message: &str, pos: usize) -> String {
        let mut line = 1;
        let mut column = 1;
        let mut i = 0;
        while i < pos.min(self.text.len()) {
            let c = self.text.code(i);
            if c == 13 && self.text.code(i + 1) == 10 && i + 1 < pos {
                i += 1;
                line += 1;
                column = 1;
            } else if c == 10 || c == 13 {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            i += 1;
        }
        format!("{message} in JSON at position {pos} (line {line} column {column})")
    }

    fn unexpected(&self, pos: usize) -> String {
        let c = self.text.code(pos);
        if c == NONE {
            return "Unexpected end of JSON input".into();
        }
        if c == 34 {
            return self.at_position("Unexpected string", pos);
        }
        if c == 45 || is_digit(c) {
            return self.at_position("Unexpected number", pos);
        }
        let token = self.text.slice(pos, pos + 1);
        let len = self.text.len();
        if len <= SHORT {
            return format!(
                "Unexpected token '{token}', \"{}\" is not valid JSON",
                self.text.slice(0, len)
            );
        }
        let start = pos.saturating_sub(CONTEXT);
        let end = (pos + CONTEXT).min(len);
        let context = self.text.slice(start, end);
        let before = if start == 0 { "" } else { "..." };
        let after = if end == len { "" } else { "..." };
        format!("Unexpected token '{token}', {before}\"{context}\"{after} is not valid JSON")
    }

    fn fail_at(&self, message: &str) -> String {
        self.at_position(message, self.at)
    }

    fn value(&mut self, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(String::new());
        }
        self.skip();
        match self.code() {
            NONE => Err("Unexpected end of JSON input".into()),
            123 => self.object(depth),
            91 => self.array(depth),
            34 => self.string(),
            116 => self.literal("true"),
            102 => self.literal("false"),
            110 => self.literal("null"),
            c if c == 45 || is_digit(c) => self.number(),
            _ => Err(self.unexpected(self.at)),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), String> {
        self.at += 1;
        self.skip();
        if self.code() == 125 {
            self.at += 1;
            return Ok(());
        }
        if self.code() != 34 {
            return Err(self.fail_at("Expected property name or '}'"));
        }
        loop {
            self.string()?;
            self.skip();
            if self.code() != 58 {
                return Err(self.fail_at("Expected ':' after property name"));
            }
            self.at += 1;
            self.value(depth + 1)?;
            self.skip();
            match self.code() {
                44 => {
                    self.at += 1;
                    self.skip();
                    if self.code() != 34 {
                        return Err(self.fail_at("Expected double-quoted property name"));
                    }
                }
                125 => {
                    self.at += 1;
                    return Ok(());
                }
                _ => return Err(self.fail_at("Expected ',' or '}' after property value")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), String> {
        self.at += 1;
        self.skip();
        if self.code() == 93 {
            self.at += 1;
            return Ok(());
        }
        loop {
            self.value(depth + 1)?;
            self.skip();
            match self.code() {
                44 => self.at += 1,
                93 => {
                    self.at += 1;
                    return Ok(());
                }
                _ => return Err(self.fail_at("Expected ',' or ']' after array element")),
            }
        }
    }

    fn string(&mut self) -> Result<(), String> {
        self.at += 1;
        loop {
            let c = self.code();
            match c {
                NONE => return Err(self.fail_at("Unterminated string")),
                34 => {
                    self.at += 1;
                    return Ok(());
                }
                92 => {
                    self.at += 1;
                    match self.code() {
                        NONE => return Err("Unexpected end of JSON input".into()),
                        34 | 92 | 47 | 98 | 102 | 110 | 114 | 116 => self.at += 1,
                        117 => {
                            self.at += 1;
                            for _ in 0..4 {
                                match self.code() {
                                    NONE => return Err("Unexpected end of JSON input".into()),
                                    h if is_hex(h) => self.at += 1,
                                    _ => return Err(self.fail_at("Bad Unicode escape")),
                                }
                            }
                        }
                        _ => return Err(self.fail_at("Bad escaped character")),
                    }
                }
                c if c < 0x20 => {
                    return Err(self.fail_at("Bad control character in string literal"));
                }
                _ => self.at += 1,
            }
        }
    }

    fn digits(&mut self) {
        while is_digit(self.code()) {
            self.at += 1;
        }
    }

    fn number(&mut self) -> Result<(), String> {
        if self.code() == 45 {
            self.at += 1;
            if !is_digit(self.code()) {
                return Err(self.fail_at("No number after minus sign"));
            }
        }
        if self.code() == 48 {
            self.at += 1;
            if is_digit(self.code()) {
                return Err(self.fail_at("Unexpected number"));
            }
        } else {
            self.digits();
        }
        if self.code() == 46 {
            self.at += 1;
            if !is_digit(self.code()) {
                return Err(self.fail_at("Unterminated fractional number"));
            }
            self.digits();
        }
        if (self.code() | 32) == 101 {
            self.at += 1;
            if self.code() == 43 || self.code() == 45 {
                self.at += 1;
            }
            if !is_digit(self.code()) {
                return Err(self.fail_at("Exponent part is missing a number"));
            }
            self.digits();
        }
        Ok(())
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        for b in word.bytes() {
            if self.code() != u32::from(b) {
                return Err(self.unexpected(self.at));
            }
            self.at += 1;
        }
        Ok(())
    }

    /// `None` when V8 accepts the text; an empty message where it is nested
    /// deeper than [`MAX_DEPTH`].
    fn check(text: &Units) -> Option<String> {
        let mut v8 = V8 { text, at: 0 };
        if let Err(message) = v8.value(0) {
            return Some(message);
        }
        v8.skip();
        if v8.at < text.len() {
            return Some(v8.fail_at("Unexpected non-whitespace character after JSON"));
        }
        None
    }
}

fn location_at(text: &Units, offset: usize, mut line: usize, mut column: usize) -> (usize, usize) {
    let end = offset.min(text.len());
    for i in 0..end {
        let c = text.code(i);
        if c == 13 {
            line += 1;
            column = 1;
        } else if c == 10 {
            if i == 0 || text.code(i - 1) != 13 {
                line += 1;
                column = 1;
            }
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Where the message of a `JSON.parse` error points: engines word them
/// differently, some with an offset, some with a line and column.
fn location_from_message(
    message: &str,
    text: &Units,
    line: usize,
    column: usize,
) -> Option<(usize, usize)> {
    if let Some(at) = message.find("position ") {
        let digits: String = message[at + 9..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(offset) = digits.parse::<usize>() {
            return Some(location_at(text, offset, line, column));
        }
    }
    let lower = message.to_lowercase();
    if lower.contains("unexpected end") || lower.contains("end of json input") {
        return Some(location_at(text, text.len(), line, column));
    }
    None
}

fn json_error(message: String, line: usize, column: usize) -> Error {
    let mut error =
        Error::syntax(format!("{message} (line {line}, column {column})")).of(Kind::Usecase);
    error.line = line;
    error.column = column;
    error
}

/// Parses the body of a `json` node. `origin` gives the line and column the
/// text starts at in the diagram.
pub fn parse_ordered_json(
    text: &str,
    origin: impl Fn() -> (usize, usize),
) -> Result<OrderedJson, Error> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let walk = Units(&units);
    if let Some(message) = V8::check(&walk) {
        if message.is_empty() {
            return Err(Error::limit("Diagram is nested too deeply.").of(Kind::Usecase));
        }
        let (line, column) = origin();
        let location = location_from_message(&message, &walk, line, column).unwrap_or_else(|| {
            let offset = property_order(&walk).err().unwrap_or(0);
            location_at(&walk, offset, line, column)
        });
        return Err(json_error(
            format!("Invalid JSON: {message}"),
            location.0,
            location.1,
        ));
    }
    let value = parse_json(text).unwrap_or_default();
    if !matches!(value, Value::Map(_)) {
        let (line, column) = origin();
        return Err(json_error(
            "JSON value must have an object root".into(),
            line,
            column,
        ));
    }
    let property_order = property_order(&walk).unwrap_or_default();
    Ok(OrderedJson {
        value,
        property_order,
    })
}
