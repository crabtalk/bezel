//! The body of a `json` node: its value, and the order its keys were
//! written in. Pele's `diagrams/usecase/json.ts`, where `JSON.parse` stands
//! in: its errors are worded as V8 words them.

use indexmap::IndexMap;

use super::lexer::Source;
use crate::{
    config::Value,
    detect::Kind,
    error::Error,
    js::parse_json,
    util::chars::{is_digit, is_hex},
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

/// Whitespace between JSON tokens.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// `key` escaped as a JSON Pointer reference token.
pub(super) fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Walks JSON text that `JSON.parse` has accepted, to recover the order its
/// keys were written in, which `JSON.parse` loses for integer-like keys. On
/// text `JSON.parse` rejected it finds where, as the offset of the first
/// character that is not valid JSON. Nesting is kept on a stack of its own,
/// so depth is limited by memory and not by the call stack.
fn property_order(text: &Source) -> Result<IndexMap<String, Vec<String>>, usize> {
    let mut order: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut created: Vec<String> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let n = text.len();
    let mut at = 0;
    let mut pointer = String::new();
    let mut state = State::Value;

    let skip = |at: &mut usize| {
        while text.matches(*at, is_space) {
            *at += 1;
        }
    };

    let read_string = |at: &mut usize, decode: bool| -> Result<String, usize> {
        *at += 1;
        let mut value: Vec<u16> = Vec::new();
        while let Some(ch) = text.char(*at) {
            let start = *at;
            *at += 1;
            if ch == '"' {
                return Ok(String::from_utf16_lossy(&value));
            }
            if ch < ' ' {
                return Err(start);
            }
            if ch != '\\' {
                if decode {
                    value.push(text.units[start]);
                }
                continue;
            }
            let escape_at = *at;
            let escape = text.char(*at);
            *at += 1;
            let decoded: u16 = match escape {
                Some(c @ ('"' | '\\' | '/')) => c as u16,
                Some('b') => 0x08,
                Some('f') => 0x0c,
                Some('n') => 0x0a,
                Some('r') => 0x0d,
                Some('t') => 0x09,
                Some('u') => {
                    if *at + 4 > n || !(0..4).all(|k| text.matches(*at + k, is_hex)) {
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
        for (k, c) in word.chars().enumerate() {
            if !text.is(*at + k, c) {
                return Err(*at + k);
            }
        }
        *at += word.len();
        Ok(())
    };

    let number = |at: &mut usize| -> Result<(), usize> {
        let digits = |at: &mut usize| {
            while text.matches(*at, is_digit) {
                *at += 1;
            }
        };
        if text.is(*at, '-') {
            *at += 1;
        }
        if text.is(*at, '0') {
            *at += 1;
        } else if text.matches(*at, is_digit) {
            digits(at);
        } else {
            return Err(*at);
        }
        if text.is(*at, '.') {
            *at += 1;
            if !text.matches(*at, is_digit) {
                return Err(*at);
            }
            digits(at);
        }
        if text.matches(*at, |c| matches!(c, 'e' | 'E')) {
            *at += 1;
            if text.matches(*at, |c| matches!(c, '+' | '-')) {
                *at += 1;
            }
            if !text.matches(*at, is_digit) {
                return Err(*at);
            }
            digits(at);
        }
        Ok(())
    };

    skip(&mut at);
    loop {
        if state == State::Value {
            skip(&mut at);
            let c = text.char(at);
            state = State::After;
            if c == Some('{') {
                at += 1;
                order.insert(pointer.clone(), Vec::new());
                created.push(pointer.clone());
                skip(&mut at);
                if text.is(at, '}') {
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
            } else if c == Some('[') {
                at += 1;
                skip(&mut at);
                if text.is(at, ']') {
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
            } else if c == Some('"') {
                read_string(&mut at, false)?;
            } else if c == Some('t') {
                literal(&mut at, "true")?;
            } else if c == Some('f') {
                literal(&mut at, "false")?;
            } else if c == Some('n') {
                literal(&mut at, "null")?;
            } else if c.is_some_and(|c| c == '-' || c.is_ascii_digit()) {
                number(&mut at)?;
            } else {
                return Err(at);
            }
        } else if state == State::Key {
            if !text.is(at, '"') {
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
            if !text.is(at, ':') {
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
            if text.is(at, if frame.object { '}' } else { ']' }) {
                at += 1;
                stack.pop();
            } else if !text.is(at, ',') {
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
    text: &'a Source,
    at: usize,
}

impl V8<'_> {
    fn char(&self) -> Option<char> {
        self.text.char(self.at)
    }

    fn is(&self, c: char) -> bool {
        self.text.is(self.at, c)
    }

    fn at_digit(&self) -> bool {
        self.text.matches(self.at, is_digit)
    }

    fn skip(&mut self) {
        while self.text.matches(self.at, is_space) {
            self.at += 1;
        }
    }

    fn at_position(&self, message: &str, pos: usize) -> String {
        let (line, column) = self.text.line_column(pos.min(self.text.len()));
        format!("{message} in JSON at position {pos} (line {line} column {column})")
    }

    fn unexpected(&self, pos: usize) -> String {
        let c = match self.text.char(pos) {
            None => return "Unexpected end of JSON input".into(),
            Some('"') => return self.at_position("Unexpected string", pos),
            Some(c) => c,
        };
        if c == '-' || c.is_ascii_digit() {
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
        match self.char() {
            None => Err("Unexpected end of JSON input".into()),
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => self.string(),
            Some('t') => self.literal("true"),
            Some('f') => self.literal("false"),
            Some('n') => self.literal("null"),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.unexpected(self.at)),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), String> {
        self.at += 1;
        self.skip();
        if self.is('}') {
            self.at += 1;
            return Ok(());
        }
        if !self.is('"') {
            return Err(self.fail_at("Expected property name or '}'"));
        }
        loop {
            self.string()?;
            self.skip();
            if !self.is(':') {
                return Err(self.fail_at("Expected ':' after property name"));
            }
            self.at += 1;
            self.value(depth + 1)?;
            self.skip();
            match self.char() {
                Some(',') => {
                    self.at += 1;
                    self.skip();
                    if !self.is('"') {
                        return Err(self.fail_at("Expected double-quoted property name"));
                    }
                }
                Some('}') => {
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
        if self.is(']') {
            self.at += 1;
            return Ok(());
        }
        loop {
            self.value(depth + 1)?;
            self.skip();
            match self.char() {
                Some(',') => self.at += 1,
                Some(']') => {
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
            match self.char() {
                None => return Err(self.fail_at("Unterminated string")),
                Some('"') => {
                    self.at += 1;
                    return Ok(());
                }
                Some('\\') => {
                    self.at += 1;
                    match self.char() {
                        None => return Err("Unexpected end of JSON input".into()),
                        Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => self.at += 1,
                        Some('u') => {
                            self.at += 1;
                            for _ in 0..4 {
                                match self.char() {
                                    None => return Err("Unexpected end of JSON input".into()),
                                    Some(h) if h.is_ascii_hexdigit() => self.at += 1,
                                    Some(_) => return Err(self.fail_at("Bad Unicode escape")),
                                }
                            }
                        }
                        Some(_) => return Err(self.fail_at("Bad escaped character")),
                    }
                }
                Some(c) if c < ' ' => {
                    return Err(self.fail_at("Bad control character in string literal"));
                }
                Some(_) => self.at += 1,
            }
        }
    }

    fn digits(&mut self) {
        while self.at_digit() {
            self.at += 1;
        }
    }

    fn number(&mut self) -> Result<(), String> {
        if self.is('-') {
            self.at += 1;
            if !self.at_digit() {
                return Err(self.fail_at("No number after minus sign"));
            }
        }
        if self.is('0') {
            self.at += 1;
            if self.at_digit() {
                return Err(self.fail_at("Unexpected number"));
            }
        } else {
            self.digits();
        }
        if self.is('.') {
            self.at += 1;
            if !self.at_digit() {
                return Err(self.fail_at("Unterminated fractional number"));
            }
            self.digits();
        }
        if self.is('e') || self.is('E') {
            self.at += 1;
            if self.is('+') || self.is('-') {
                self.at += 1;
            }
            if !self.at_digit() {
                return Err(self.fail_at("Exponent part is missing a number"));
            }
            self.digits();
        }
        Ok(())
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        for c in word.chars() {
            if !self.is(c) {
                return Err(self.unexpected(self.at));
            }
            self.at += 1;
        }
        Ok(())
    }

    /// `None` when V8 accepts the text; an empty message where it is nested
    /// deeper than [`MAX_DEPTH`].
    fn check(text: &Source) -> Option<String> {
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

/// Where the message of a `JSON.parse` error points: engines word them
/// differently, some with an offset, some with a line and column. `origin`
/// is where `text` starts in the diagram.
fn location_from_message(
    message: &str,
    text: &Source,
    origin: (usize, usize),
) -> Option<(usize, usize)> {
    if let Some(at) = message.find("position ") {
        let digits: String = message[at + 9..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(offset) = digits.parse::<usize>() {
            return Some(text.line_column_from(offset, origin));
        }
    }
    let lower = message.to_lowercase();
    if lower.contains("unexpected end") || lower.contains("end of json input") {
        return Some(text.line_column_from(text.len(), origin));
    }
    None
}

fn json_error(message: String, (line, column): (usize, usize)) -> Error {
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
    let source = Source::new(text);
    if let Some(message) = V8::check(&source) {
        if message.is_empty() {
            return Err(Error::limit("Diagram is nested too deeply.").of(Kind::Usecase));
        }
        let origin = origin();
        let location = location_from_message(&message, &source, origin).unwrap_or_else(|| {
            let offset = property_order(&source).err().unwrap_or(0);
            source.line_column_from(offset, origin)
        });
        return Err(json_error(format!("Invalid JSON: {message}"), location));
    }
    let value = parse_json(text).unwrap_or_default();
    if !matches!(value, Value::Object(_)) {
        return Err(json_error(
            "JSON value must have an object root".into(),
            origin(),
        ));
    }
    let property_order = property_order(&source).unwrap_or_default();
    Ok(OrderedJson {
        value,
        property_order,
    })
}
