//! Pele's `diagrams/xychart/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const NEWLINE: u8 = 2;
pub const TITLE: u8 = 3;
pub const ACC_TITLE: u8 = 4;
pub const ACC_TITLE_VALUE: u8 = 5;
pub const ACC_DESCR: u8 = 6;
pub const ACC_DESCR_VALUE: u8 = 7;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 8;
pub const XYCHART: u8 = 9;
pub const CHART_ORIENTATION: u8 = 10;
pub const X_AXIS: u8 = 11;
pub const Y_AXIS: u8 = 12;
pub const ARROW_DELIMITER: u8 = 13;
pub const LINE: u8 = 14;
pub const BAR: u8 = 15;
pub const NUMBER_WITH_DECIMAL: u8 = 16;
pub const SQUARE_BRACES_START: u8 = 17;
pub const SQUARE_BRACES_END: u8 = 18;
pub const STR: u8 = 19;
pub const ALPHA: u8 = 20;
pub const COLON: u8 = 21;
pub const PLUS: u8 = 22;
pub const COMMA: u8 = 23;
pub const EQUALS: u8 = 24;
pub const MULT: u8 = 25;
pub const BRKT: u8 = 26;
pub const UNDERSCORE: u8 = 27;
pub const DOT: u8 = 28;
pub const AMP: u8 = 29;
pub const MINUS: u8 = 30;
pub const NUM: u8 = 31;
pub const SEMI: u8 = 32;
pub const EOF: u8 = 33;

pub const TOKEN_NAMES: [&str; 34] = [
    "$end",
    "INVALID",
    "NEWLINE",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "XYCHART",
    "CHART_ORIENTATION",
    "X_AXIS",
    "Y_AXIS",
    "ARROW_DELIMITER",
    "LINE",
    "BAR",
    "NUMBER_WITH_DECIMAL",
    "SQUARE_BRACES_START",
    "SQUARE_BRACES_END",
    "STR",
    "ALPHA",
    "COLON",
    "PLUS",
    "COMMA",
    "EQUALS",
    "MULT",
    "BRKT",
    "UNDERSCORE",
    "DOT",
    "AMP",
    "MINUS",
    "NUM",
    "SEMI",
    "EOF",
];
