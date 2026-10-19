//! Pele's `diagrams/er/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const ER_DIAGRAM: u8 = 2;
pub const EOF: u8 = 3;
pub const NEWLINE: u8 = 4;
pub const COLON: u8 = 5;
pub const STYLE_SEPARATOR: u8 = 6;
pub const BLOCK_START: u8 = 7;
pub const BLOCK_STOP: u8 = 8;
pub const SQS: u8 = 9;
pub const SQE: u8 = 10;
pub const ACC_TITLE: u8 = 11;
pub const ACC_TITLE_VALUE: u8 = 12;
pub const ACC_DESCR: u8 = 13;
pub const ACC_DESCR_VALUE: u8 = 14;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 15;
pub const END_KW: u8 = 16;
pub const SUBGRAPH: u8 = 17;
pub const DIRECTION_TB: u8 = 18;
pub const DIRECTION_BT: u8 = 19;
pub const DIRECTION_RL: u8 = 20;
pub const DIRECTION_LR: u8 = 21;
pub const CLASSDEF: u8 = 22;
pub const UNICODE_TEXT: u8 = 23;
pub const STYLE_TEXT: u8 = 24;
pub const COMMA: u8 = 25;
pub const CLASS: u8 = 26;
pub const STYLE: u8 = 27;
pub const SEMI: u8 = 28;
pub const NUM: u8 = 29;
pub const BRKT: u8 = 30;
pub const ENTITY_NAME: u8 = 31;
pub const DECIMAL_NUM: u8 = 32;
pub const ENTITY_ONE: u8 = 33;
pub const ATTRIBUTE_WORD: u8 = 34;
pub const ATTRIBUTE_KEY: u8 = 35;
pub const COMMENT: u8 = 36;
pub const ZERO_OR_ONE: u8 = 37;
pub const ZERO_OR_MORE: u8 = 38;
pub const ONE_OR_MORE: u8 = 39;
pub const ONLY_ONE: u8 = 40;
pub const MD_PARENT: u8 = 41;
pub const NON_IDENTIFYING: u8 = 42;
pub const IDENTIFYING: u8 = 43;
pub const WORD: u8 = 44;
/// A character no rule names; the grammar gives it meaning only as `?` and
/// `,` inside a block.
pub const CHAR: u8 = 45;

pub const TOKEN_NAMES: [&str; 46] = [
    "$end",
    "INVALID",
    "ER_DIAGRAM",
    "EOF",
    "NEWLINE",
    "COLON",
    "STYLE_SEPARATOR",
    "BLOCK_START",
    "BLOCK_STOP",
    "SQS",
    "SQE",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "END",
    "SUBGRAPH",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "CLASSDEF",
    "UNICODE_TEXT",
    "STYLE_TEXT",
    "COMMA",
    "CLASS",
    "STYLE",
    "SEMI",
    "NUM",
    "BRKT",
    "ENTITY_NAME",
    "DECIMAL_NUM",
    "ENTITY_ONE",
    "ATTRIBUTE_WORD",
    "ATTRIBUTE_KEY",
    "COMMENT",
    "ZERO_OR_ONE",
    "ZERO_OR_MORE",
    "ONE_OR_MORE",
    "ONLY_ONE",
    "MD_PARENT",
    "NON_IDENTIFYING",
    "IDENTIFYING",
    "WORD",
    "CHAR",
];
