//! Pele's `diagrams/class/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const CLASS_DIAGRAM: u8 = 2;
pub const NEWLINE: u8 = 3;
pub const EOF: u8 = 4;
pub const SQS: u8 = 5;
pub const STR: u8 = 6;
pub const SQE: u8 = 7;
pub const DOT: u8 = 8;
pub const GENERICTYPE: u8 = 9;
pub const LABEL: u8 = 10;
pub const ACC_TITLE: u8 = 11;
pub const ACC_TITLE_VALUE: u8 = 12;
pub const ACC_DESCR: u8 = 13;
pub const ACC_DESCR_VALUE: u8 = 14;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 15;
pub const STRUCT_START: u8 = 16;
pub const STRUCT_STOP: u8 = 17;
pub const NAMESPACE: u8 = 18;
pub const STYLE_SEPARATOR: u8 = 19;
pub const ANNOTATION_START: u8 = 20;
pub const ANNOTATION_END: u8 = 21;
pub const CLASS: u8 = 22;
pub const MEMBER: u8 = 23;
pub const NOTE_FOR: u8 = 24;
pub const NOTE: u8 = 25;
pub const CLASSDEF: u8 = 26;
pub const ALPHA: u8 = 27;
pub const COMMA: u8 = 28;
pub const DIRECTION_TB: u8 = 29;
pub const DIRECTION_BT: u8 = 30;
pub const DIRECTION_RL: u8 = 31;
pub const DIRECTION_LR: u8 = 32;
pub const AGGREGATION: u8 = 33;
pub const EXTENSION: u8 = 34;
pub const COMPOSITION: u8 = 35;
pub const DEPENDENCY: u8 = 36;
pub const LOLLIPOP: u8 = 37;
pub const LINE: u8 = 38;
pub const DOTTED_LINE: u8 = 39;
pub const CALLBACK: u8 = 40;
pub const LINK: u8 = 41;
pub const LINK_TARGET: u8 = 42;
pub const CLICK: u8 = 43;
pub const CALLBACK_NAME: u8 = 44;
pub const CALLBACK_ARGS: u8 = 45;
pub const HREF: u8 = 46;
pub const STYLE: u8 = 47;
pub const CSSCLASS: u8 = 48;
pub const COLON: u8 = 49;
pub const BRKT: u8 = 50;
pub const PCT: u8 = 51;
pub const MINUS: u8 = 52;
pub const UNICODE_TEXT: u8 = 53;
pub const BQUOTE_STR: u8 = 54;
pub const EDGE_STATE: u8 = 55;
pub const EOF_IN_STRUCT: u8 = 56;
pub const OPEN_IN_STRUCT: u8 = 57;
pub const PLUS: u8 = 58;
pub const EQUALS: u8 = 59;
pub const PUNCTUATION: u8 = 60;

pub const COUNT: usize = 61;

pub const TOKEN_NAMES: [&str; COUNT] = [
    "$end",
    "INVALID",
    "CLASS_DIAGRAM",
    "NEWLINE",
    "EOF",
    "SQS",
    "STR",
    "SQE",
    "DOT",
    "GENERICTYPE",
    "LABEL",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "STRUCT_START",
    "STRUCT_STOP",
    "NAMESPACE",
    "STYLE_SEPARATOR",
    "ANNOTATION_START",
    "ANNOTATION_END",
    "CLASS",
    "MEMBER",
    "NOTE_FOR",
    "NOTE",
    "CLASSDEF",
    "ALPHA",
    "COMMA",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "AGGREGATION",
    "EXTENSION",
    "COMPOSITION",
    "DEPENDENCY",
    "LOLLIPOP",
    "LINE",
    "DOTTED_LINE",
    "CALLBACK",
    "LINK",
    "LINK_TARGET",
    "CLICK",
    "CALLBACK_NAME",
    "CALLBACK_ARGS",
    "HREF",
    "STYLE",
    "CSSCLASS",
    "COLON",
    "BRKT",
    "PCT",
    "MINUS",
    "UNICODE_TEXT",
    "BQUOTE_STR",
    "EDGE_STATE",
    "EOF_IN_STRUCT",
    "OPEN_IN_STRUCT",
    "PLUS",
    "EQUALS",
    "PUNCTUATION",
];
