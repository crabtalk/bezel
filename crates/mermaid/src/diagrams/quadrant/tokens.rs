//! Pele's `diagrams/quadrant/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const NEWLINE: u8 = 2;
pub const TITLE: u8 = 3;
pub const TITLE_VALUE: u8 = 4;
pub const ACC_TITLE: u8 = 5;
pub const ACC_TITLE_VALUE: u8 = 6;
pub const ACC_DESCR: u8 = 7;
pub const ACC_DESCR_VALUE: u8 = 8;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 9;
pub const X_AXIS: u8 = 10;
pub const Y_AXIS: u8 = 11;
pub const DELIMITER: u8 = 12;
pub const QUADRANT_1: u8 = 13;
pub const QUADRANT_2: u8 = 14;
pub const QUADRANT_3: u8 = 15;
pub const QUADRANT_4: u8 = 16;
pub const CLASSDEF: u8 = 17;
pub const MD_STR: u8 = 18;
pub const STR: u8 = 19;
pub const CLASS_NAME: u8 = 20;
pub const POINT_START: u8 = 21;
pub const POINT_X: u8 = 22;
pub const POINT_Y: u8 = 23;
pub const QUADRANT: u8 = 24;
pub const ALPHA: u8 = 25;
pub const UNICODE_TEXT: u8 = 26;
pub const COLON: u8 = 27;
pub const PLUS: u8 = 28;
pub const COMMA: u8 = 29;
pub const EQUALS: u8 = 30;
pub const MULT: u8 = 31;
pub const BRKT: u8 = 32;
pub const UNDERSCORE: u8 = 33;
pub const DOT: u8 = 34;
pub const AMP: u8 = 35;
pub const MINUS: u8 = 36;
pub const NUM: u8 = 37;
pub const SPACE: u8 = 38;
pub const SEMI: u8 = 39;
pub const PUNCTUATION: u8 = 40;
pub const EOF: u8 = 41;

pub const TOKEN_NAMES: [&str; 42] = [
    "$end",
    "INVALID",
    "NEWLINE",
    "title",
    "title_value",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "X-AXIS",
    "Y-AXIS",
    "AXIS-TEXT-DELIMITER",
    "QUADRANT_1",
    "QUADRANT_2",
    "QUADRANT_3",
    "QUADRANT_4",
    "CLASSDEF",
    "MD_STR",
    "STR",
    "class_name",
    "point_start",
    "point_x",
    "point_y",
    "QUADRANT",
    "ALPHA",
    "UNICODE_TEXT",
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
    "SPACE",
    "SEMI",
    "PUNCTUATION",
    "EOF",
];
