//! Mermaid's token types, in an order that makes its token categories
//! ranges: WORD is USECASE to IDENTIFIER, and LABEL_TEXT is USECASE to
//! LABEL_SYMBOL. Pele's `diagrams/usecase/tokens.ts`.

/// The use case lexer's token types.
pub struct T;

impl T {
    pub const EOF: usize = 0;
    pub const USECASE: usize = 1;
    pub const ACTOR: usize = 2;
    pub const SYSTEM_BOUNDARY: usize = 3;
    pub const END: usize = 4;
    pub const DIRECTION: usize = 5;
    pub const TD: usize = 6;
    pub const TB: usize = 7;
    pub const BT: usize = 8;
    pub const LR: usize = 9;
    pub const RL: usize = 10;
    pub const NOTE: usize = 11;
    pub const FOR: usize = 12;
    pub const JSON: usize = 13;
    pub const CLASS_DEF: usize = 14;
    pub const CLASS: usize = 15;
    pub const STYLE: usize = 16;
    pub const INCLUDE: usize = 17;
    pub const EXTEND: usize = 18;
    pub const TRUE: usize = 19;
    pub const FALSE: usize = 20;
    pub const IDENTIFIER: usize = 21;
    pub const NUMBER: usize = 22;
    pub const AT: usize = 23;
    pub const COMMA: usize = 24;
    pub const COLON: usize = 25;
    pub const HASH_COLOR: usize = 26;
    pub const CSS_IDENTIFIER: usize = 27;
    pub const CSS_ESCAPED_COMMA: usize = 28;
    pub const DASH: usize = 29;
    pub const DOT: usize = 30;
    pub const PERCENT: usize = 31;
    pub const CSS_PUNCTUATION: usize = 32;
    pub const LABEL_PUNCTUATION: usize = 33;
    pub const LABEL_SYMBOL: usize = 34;
    pub const PLAIN_STRING: usize = 35;
    pub const MARKDOWN_STRING: usize = 36;
    pub const UNCLOSED_MARKDOWN_STRING: usize = 37;
    pub const COMMENT: usize = 38;
    pub const NEWLINE: usize = 39;
    pub const ACC_DESCR_BLOCK: usize = 40;
    pub const ACC_TITLE_LINE: usize = 41;
    pub const ACC_DESCR_LINE: usize = 42;
    pub const JSON_DECLARATION_START: usize = 43;
    pub const JSON_OBJECT_LITERAL: usize = 44;
    pub const UNCLOSED_JSON_OBJECT_LITERAL: usize = 45;
    pub const GENERALIZATION: usize = 46;
    pub const DEPENDENCY_ARROW: usize = 47;
    pub const STEREOTYPE_START: usize = 48;
    pub const STEREOTYPE_END: usize = 49;
    pub const STEREOTYPE_TEXT: usize = 50;
    pub const UNCLOSED_STEREOTYPE_TEXT: usize = 51;
    pub const CLASS_SEPARATOR: usize = 52;
    pub const FORWARD_SOLID: usize = 53;
    pub const BACKWARD_SOLID: usize = 54;
    pub const FORWARD_CIRCLE: usize = 55;
    pub const BACKWARD_CIRCLE: usize = 56;
    pub const FORWARD_CROSS: usize = 57;
    pub const BACKWARD_CROSS: usize = 58;
    pub const MARKERLESS_SOLID: usize = 59;
    pub const METADATA_START: usize = 60;
    pub const LBRACE: usize = 61;
    pub const RBRACE: usize = 62;
    pub const LBRACKET: usize = 63;
    pub const RBRACKET: usize = 64;
    pub const LPAREN: usize = 65;
    pub const RPAREN: usize = 66;

    /// The token type named `name`.
    pub fn named(name: &str) -> Option<usize> {
        TOKEN_NAMES.iter().position(|&n| n == name)
    }
}

pub const TOKEN_NAMES: [&str; 67] = [
    "EOF",
    "USECASE",
    "ACTOR",
    "SYSTEM_BOUNDARY",
    "END",
    "DIRECTION",
    "TD",
    "TB",
    "BT",
    "LR",
    "RL",
    "NOTE",
    "FOR",
    "JSON",
    "CLASS_DEF",
    "CLASS",
    "STYLE",
    "INCLUDE",
    "EXTEND",
    "TRUE",
    "FALSE",
    "IDENTIFIER",
    "NUMBER",
    "AT",
    "COMMA",
    "COLON",
    "HASH_COLOR",
    "CSS_IDENTIFIER",
    "CSS_ESCAPED_COMMA",
    "DASH",
    "DOT",
    "PERCENT",
    "CSS_PUNCTUATION",
    "LABEL_PUNCTUATION",
    "LABEL_SYMBOL",
    "PLAIN_STRING",
    "MARKDOWN_STRING",
    "UNCLOSED_MARKDOWN_STRING",
    "COMMENT",
    "NEWLINE",
    "ACC_DESCR_BLOCK",
    "ACC_TITLE_LINE",
    "ACC_DESCR_LINE",
    "JSON_DECLARATION_START",
    "JSON_OBJECT_LITERAL",
    "UNCLOSED_JSON_OBJECT_LITERAL",
    "GENERALIZATION",
    "DEPENDENCY_ARROW",
    "STEREOTYPE_START",
    "STEREOTYPE_END",
    "STEREOTYPE_TEXT",
    "UNCLOSED_STEREOTYPE_TEXT",
    "CLASS_SEPARATOR",
    "FORWARD_SOLID",
    "BACKWARD_SOLID",
    "FORWARD_CIRCLE",
    "BACKWARD_CIRCLE",
    "FORWARD_CROSS",
    "BACKWARD_CROSS",
    "MARKERLESS_SOLID",
    "METADATA_START",
    "LBRACE",
    "RBRACE",
    "LBRACKET",
    "RBRACKET",
    "LPAREN",
    "RPAREN",
];
