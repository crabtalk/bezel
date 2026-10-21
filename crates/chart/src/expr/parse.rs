//! Source text to a syntax tree, by JavaScript's grammar and precedence.

use crate::expr::{Error, RegExp, Value, function::Function};
use gpui::SharedString;
use js::{
    number::{self, Number},
    string,
};
use std::f64::consts;

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Literal(Value),
    Array(Vec<Node>),
    Object(Vec<(SharedString, Node)>),
    Datum,
    Param(SharedString),
    Member(Box<Node>, Box<Node>),
    Unary(Unary, Box<Node>),
    Binary(Binary, Box<Node>, Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Conditional(Box<Node>, Box<Node>, Box<Node>),
    Call(Function, Vec<Node>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Unary {
    Not,
    Negate,
    Plus,
    BitNot,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Gt,
    Le,
    Ge,
}

/// Binary operators from loosest to tightest, each level left-associative.
/// `&&` and `||` are the two loosest.
const LEVELS: [&[(&str, Binary)]; 8] = [
    &[("|", Binary::BitOr)],
    &[("^", Binary::BitXor)],
    &[("&", Binary::BitAnd)],
    &[
        ("===", Binary::StrictEq),
        ("!==", Binary::StrictNe),
        ("==", Binary::Eq),
        ("!=", Binary::Ne),
    ],
    &[
        ("<=", Binary::Le),
        (">=", Binary::Ge),
        ("<", Binary::Lt),
        (">", Binary::Gt),
    ],
    &[
        (">>>", Binary::UShr),
        ("<<", Binary::Shl),
        (">>", Binary::Shr),
    ],
    &[("+", Binary::Add), ("-", Binary::Sub)],
    &[("*", Binary::Mul), ("/", Binary::Div), ("%", Binary::Rem)],
];

/// Punctuators, longest first so a scan takes the longest that matches.
const PUNCTUATORS: [&str; 47] = [
    ">>>=", "===", "!==", ">>>", "<<=", ">>=", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>",
    "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "+", "-", "*", "/", "%", "<", ">",
    "&", "|", "^", "!", "~", "?", ":", ".", ",", "(", ")", "[", "]", "{", "}", "=",
];

/// Words JavaScript reserves, which are never a param.
const RESERVED: [&str; 37] = [
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "let",
    "static",
    "yield",
    "await",
];

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Name(String),
    Punct(&'static str),
    End,
}

pub(crate) struct Parser<'a> {
    source: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source, at: 0 }
    }

    /// The whole source as one expression.
    pub(crate) fn expression(mut self) -> Result<Node, Error> {
        let node = self.conditional()?;
        match self.next()? {
            (Token::End, _) => Ok(node),
            (_, at) => Err(syntax(at, "unexpected token")),
        }
    }

    fn conditional(&mut self) -> Result<Node, Error> {
        let test = self.or()?;
        if !self.eat("?")? {
            return Ok(test);
        }
        let then = self.conditional()?;
        self.expect(":")?;
        let otherwise = self.conditional()?;
        Ok(Node::Conditional(
            Box::new(test),
            Box::new(then),
            Box::new(otherwise),
        ))
    }

    fn or(&mut self) -> Result<Node, Error> {
        let mut left = self.and()?;
        while self.eat("||")? {
            left = Node::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Node, Error> {
        let mut left = self.binary(0)?;
        while self.eat("&&")? {
            left = Node::And(Box::new(left), Box::new(self.binary(0)?));
        }
        Ok(left)
    }

    fn binary(&mut self, level: usize) -> Result<Node, Error> {
        let Some(operators) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut left = self.binary(level + 1)?;
        'operators: loop {
            let (token, _) = self.peek()?;
            for (symbol, op) in operators.iter() {
                if matches!(token, Token::Punct(p) if p == *symbol) {
                    self.next()?;
                    let right = self.binary(level + 1)?;
                    left = Node::Binary(*op, Box::new(left), Box::new(right));
                    continue 'operators;
                }
            }
            return Ok(left);
        }
    }

    fn unary(&mut self) -> Result<Node, Error> {
        let (token, _) = self.peek()?;
        let op = match token {
            Token::Punct("!") => Unary::Not,
            Token::Punct("-") => Unary::Negate,
            Token::Punct("+") => Unary::Plus,
            Token::Punct("~") => Unary::BitNot,
            _ => return self.postfix(),
        };
        self.next()?;
        Ok(Node::Unary(op, Box::new(self.unary()?)))
    }

    fn postfix(&mut self) -> Result<Node, Error> {
        let mut node = self.primary()?;
        loop {
            let (token, at) = self.peek()?;
            match token {
                Token::Punct(".") => {
                    self.next()?;
                    match self.next()? {
                        (Token::Name(name), _) => {
                            node =
                                Node::Member(Box::new(node), Box::new(Node::Literal(name.into())));
                        }
                        (_, at) => return Err(syntax(at, "expected a property name")),
                    }
                }
                Token::Punct("[") => {
                    self.next()?;
                    let property = self.conditional()?;
                    self.expect("]")?;
                    node = Node::Member(Box::new(node), Box::new(property));
                }
                Token::Punct("(") => return Err(syntax(at, "only a function name can be called")),
                _ => return Ok(node),
            }
        }
    }

    fn primary(&mut self) -> Result<Node, Error> {
        self.skip_space();
        if self.source[self.at..].starts_with('/') {
            return self.regexp();
        }
        let (token, at) = self.next()?;
        Ok(match token {
            Token::Number(n) => Node::Literal(Value::Number(n)),
            Token::String(s) => Node::Literal(s.into()),
            Token::Punct("(") => {
                let node = self.conditional()?;
                self.expect(")")?;
                node
            }
            Token::Punct("[") => Node::Array(self.list("]")?),
            Token::Punct("{") => self.object()?,
            Token::Name(name) => {
                if self.eat("(")? {
                    let function = name
                        .parse::<Function>()
                        .map_err(|()| Error::Function(name.clone()))?;
                    let args = self.list(")")?;
                    if matches!(function, Function::If) && args.len() != 3 {
                        return Err(syntax(at, "if takes a test, a then and an else"));
                    }
                    return Ok(Node::Call(function, args));
                }
                match name.as_str() {
                    "true" => Node::Literal(Value::Bool(true)),
                    "false" => Node::Literal(Value::Bool(false)),
                    "null" => Node::Literal(Value::Null),
                    "datum" => Node::Datum,
                    _ => match constant(&name) {
                        Some(n) => Node::Literal(Value::Number(n)),
                        None if RESERVED.contains(&name.as_str()) => {
                            return Err(syntax(at, "reserved word"));
                        }
                        None => Node::Param(name.into()),
                    },
                }
            }
            Token::End => return Err(syntax(at, "unexpected end of input")),
            Token::Punct(_) => return Err(syntax(at, "unexpected token")),
        })
    }

    /// Comma-separated expressions up to `close`, which may follow a trailing
    /// comma.
    fn list(&mut self, close: &'static str) -> Result<Vec<Node>, Error> {
        let mut items = Vec::new();
        loop {
            if self.eat(close)? {
                return Ok(items);
            }
            items.push(self.conditional()?);
            if !self.eat(",")? {
                self.expect(close)?;
                return Ok(items);
            }
        }
    }

    fn object(&mut self) -> Result<Node, Error> {
        let mut fields: Vec<(SharedString, Node)> = Vec::new();
        loop {
            if self.eat("}")? {
                break;
            }
            let key: SharedString = match self.next()? {
                (Token::Name(name), _) => name.into(),
                (Token::String(s), _) => s.into(),
                (Token::Number(n), _) => Value::Number(n).to_string().into(),
                (_, at) => return Err(syntax(at, "expected a property name")),
            };
            self.expect(":")?;
            let value = self.conditional()?;
            match fields.iter_mut().find(|(k, _)| *k == key) {
                Some((_, slot)) => *slot = value,
                None => fields.push((key, value)),
            }
            if !self.eat(",")? {
                self.expect("}")?;
                break;
            }
        }
        Ok(Node::Object(fields))
    }

    /// A regular expression literal; the cursor is on its opening `/`.
    fn regexp(&mut self) -> Result<Node, Error> {
        let start = self.at;
        let mut chars = self.source[start + 1..].char_indices();
        let mut class = false;
        let end = loop {
            match chars.next() {
                None | Some((_, '\n' | '\r')) => {
                    return Err(syntax(start, "unterminated regular expression"));
                }
                Some((_, '\\')) => {
                    chars.next();
                }
                Some((_, '[')) => class = true,
                Some((_, ']')) => class = false,
                Some((i, '/')) if !class => break start + 1 + i,
                Some(_) => {}
            }
        };
        let source = &self.source[start + 1..end];
        let flags_end = self.source[end + 1..]
            .find(|c: char| !is_name_char(c))
            .map_or(self.source.len(), |i| end + 1 + i);
        let flags = &self.source[end + 1..flags_end];
        self.at = flags_end;
        let regexp =
            RegExp::new(source, flags).map_err(|error| syntax(start, &error.to_string()))?;
        Ok(Node::Literal(regexp.into()))
    }

    fn eat(&mut self, symbol: &str) -> Result<bool, Error> {
        let (token, _) = self.peek()?;
        let hit = matches!(token, Token::Punct(p) if p == symbol);
        if hit {
            self.next()?;
        }
        Ok(hit)
    }

    fn expect(&mut self, symbol: &str) -> Result<(), Error> {
        let at = self.at;
        if self.eat(symbol)? {
            Ok(())
        } else {
            Err(syntax(at, &format!("expected `{symbol}`")))
        }
    }

    fn peek(&mut self) -> Result<(Token, usize), Error> {
        let at = self.at;
        let token = self.next();
        self.at = at;
        token
    }

    fn skip_space(&mut self) {
        let rest = &self.source[self.at..];
        self.at += rest.len() - string::trim_start(rest).len();
    }

    /// The next token and the offset it starts at.
    fn next(&mut self) -> Result<(Token, usize), Error> {
        self.skip_space();
        let at = self.at;
        let rest = &self.source[at..];
        let Some(c) = rest.chars().next() else {
            return Ok((Token::End, at));
        };
        let token = if c.is_ascii_digit()
            || c == '.' && rest[1..].starts_with(|c: char| c.is_ascii_digit())
        {
            let (n, len) = number(rest).ok_or_else(|| syntax(at, "invalid number"))?;
            if rest[len..].starts_with(is_name_char) {
                return Err(syntax(at + len, "identifier directly after number"));
            }
            self.at += len;
            Token::Number(n)
        } else if c == '"' || c == '\'' {
            let (s, len) = string(rest).ok_or_else(|| syntax(at, "unterminated string"))?;
            self.at += len;
            Token::String(s)
        } else if is_name_start(c) {
            let len = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
            self.at += len;
            Token::Name(rest[..len].to_string())
        } else if let Some(p) = PUNCTUATORS.iter().find(|p| rest.starts_with(**p)) {
            self.at += p.len();
            Token::Punct(p)
        } else {
            return Err(syntax(at, "unexpected character"));
        };
        Ok((token, at))
    }
}

fn syntax(at: usize, message: &str) -> Error {
    Error::Syntax {
        at,
        message: message.to_string(),
    }
}

fn constant(name: &str) -> Option<f64> {
    Some(match name {
        "NaN" => f64::NAN,
        "E" => consts::E,
        "LN2" => consts::LN_2,
        "LN10" => consts::LN_10,
        "LOG2E" => consts::LOG2_E,
        "LOG10E" => consts::LOG10_E,
        "PI" => consts::PI,
        "SQRT1_2" => consts::FRAC_1_SQRT_2,
        "SQRT2" => consts::SQRT_2,
        "MIN_VALUE" => 5e-324,
        "MAX_VALUE" => f64::MAX,
        _ => return None,
    })
}

fn is_name_start(c: char) -> bool {
    c == '$' || c == '_' || c.is_alphabetic()
}

fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_alphanumeric() || c == '\u{200c}' || c == '\u{200d}'
}

/// A numeric literal at the start of `text`, and its length.
fn number(text: &str) -> Option<(f64, usize)> {
    let bytes = text.as_bytes();
    if bytes[0] == b'0' {
        let radix = match bytes.get(1) {
            Some(b'x' | b'X') => Some(16),
            Some(b'o' | b'O') => Some(8),
            Some(b'b' | b'B') => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            let len = 2 + text[2..]
                .find(|c: char| !c.is_digit(radix))
                .unwrap_or(text.len() - 2);
            let n = Number::from(&text[..len]).0;
            return (!n.is_nan()).then_some((n, len));
        }
        let digits = text
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len());
        if digits > 1 && text[1..digits].bytes().all(|b| b < b'8') {
            return Some((number::parse_int(&text[1..digits], 8), digits));
        }
    }
    let len = number::decimal_prefix(text);
    Some((Number::from(&text[..len]).0, len))
}

/// A quoted string literal at the start of `text`, unescaped, and its length.
fn string(text: &str) -> Option<(String, usize)> {
    let quote = text.chars().next()?;
    let mut units: Vec<u16> = Vec::new();
    let mut chars = text.char_indices().skip(1).peekable();
    let push = |units: &mut Vec<u16>, c: char| {
        let mut buf = [0; 2];
        units.extend_from_slice(c.encode_utf16(&mut buf));
    };
    while let Some((i, c)) = chars.next() {
        match c {
            c if c == quote => return Some((String::from_utf16_lossy(&units), i + 1)),
            '\n' | '\r' => return None,
            '\\' => {
                let (_, escaped) = chars.next()?;
                match escaped {
                    'n' => units.push(0x0a),
                    'r' => units.push(0x0d),
                    't' => units.push(0x09),
                    'b' => units.push(0x08),
                    'f' => units.push(0x0c),
                    'v' => units.push(0x0b),
                    '0' if !chars.peek().is_some_and(|(_, c)| c.is_ascii_digit()) => units.push(0),
                    'x' => {
                        let hex: String = (0..2)
                            .filter_map(|_| chars.next().map(|(_, c)| c))
                            .collect();
                        units.push(
                            u16::from_str_radix(&hex, 16)
                                .ok()
                                .filter(|_| hex.len() == 2)?,
                        );
                    }
                    'u' if chars.peek().is_some_and(|(_, c)| *c == '{') => {
                        chars.next();
                        let mut hex = String::new();
                        loop {
                            match chars.next()? {
                                (_, '}') => break,
                                (_, c) => hex.push(c),
                            }
                        }
                        push(
                            &mut units,
                            char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?,
                        );
                    }
                    'u' => {
                        let hex: String = (0..4)
                            .filter_map(|_| chars.next().map(|(_, c)| c))
                            .collect();
                        units.push(
                            u16::from_str_radix(&hex, 16)
                                .ok()
                                .filter(|_| hex.len() == 4)?,
                        );
                    }
                    '\r' => {
                        if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                            chars.next();
                        }
                    }
                    '\n' | '\u{2028}' | '\u{2029}' => {}
                    other => push(&mut units, other),
                }
            }
            other => push(&mut units, other),
        }
    }
    None
}
