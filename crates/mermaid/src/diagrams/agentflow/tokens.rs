//! Pele's `diagrams/agentflow/tokens.ts`.

/// The agentflow lexer's token types.
pub struct T;

impl T {
    pub const END: usize = 0;
    pub const ERROR: usize = 1;
    pub const ACC_TITLE: usize = 2;
    pub const ACC_TITLE_VALUE: usize = 3;
    pub const ACC_DESCR: usize = 4;
    pub const ACC_DESCR_VALUE: usize = 5;
    pub const ACC_DESCR_MULTILINE_VALUE: usize = 6;
    pub const SHAPE_DATA: usize = 7;
    pub const CALLBACKNAME: usize = 8;
    pub const CALLBACKARGS: usize = 9;
    pub const MD_STR: usize = 10;
    pub const STR: usize = 11;
    pub const STYLE: usize = 12;
    pub const DEFAULT: usize = 13;
    pub const LINKSTYLE: usize = 14;
    pub const INTERPOLATE: usize = 15;
    pub const CLASSDEF: usize = 16;
    pub const CLASS: usize = 17;
    pub const HREF: usize = 18;
    pub const CLICK: usize = 19;
    pub const GRAPH: usize = 20;
    pub const FLOW: usize = 21;
    pub const CONNECTOR: usize = 22;
    pub const GLOBAL: usize = 23;
    pub const END_KW: usize = 24;
    pub const LINK_TARGET: usize = 25;
    pub const NODIR: usize = 26;
    pub const DIR: usize = 27;
    pub const DIRECTION_TB: usize = 28;
    pub const DIRECTION_BT: usize = 29;
    pub const DIRECTION_RL: usize = 30;
    pub const DIRECTION_LR: usize = 31;
    pub const DIRECTION_TD: usize = 32;
    pub const LINK_ID: usize = 33;
    pub const NUM: usize = 34;
    pub const BRKT: usize = 35;
    pub const STYLE_SEPARATOR: usize = 36;
    pub const COLON: usize = 37;
    pub const AMP: usize = 38;
    pub const SEMI: usize = 39;
    pub const COMMA: usize = 40;
    pub const MULT: usize = 41;
    pub const LINK: usize = 42;
    pub const START_LINK: usize = 43;
    pub const EDGE_TEXT: usize = 44;
    pub const ELLIPSE_END: usize = 45;
    pub const TEXT: usize = 46;
    pub const ELLIPSE_START: usize = 47;
    pub const STADIUMEND: usize = 48;
    pub const STADIUMSTART: usize = 49;
    pub const SUBROUTINEEND: usize = 50;
    pub const SUBROUTINESTART: usize = 51;
    pub const VERTEX_WITH_PROPS_START: usize = 52;
    pub const TAGEND: usize = 53;
    pub const CYLINDEREND: usize = 54;
    pub const CYLINDERSTART: usize = 55;
    pub const DOUBLECIRCLEEND: usize = 56;
    pub const DOUBLECIRCLESTART: usize = 57;
    pub const TRAPEND: usize = 58;
    pub const INVTRAPEND: usize = 59;
    pub const TRAPSTART: usize = 60;
    pub const INVTRAPSTART: usize = 61;
    pub const TAGSTART: usize = 62;
    pub const UP: usize = 63;
    pub const SEP: usize = 64;
    pub const DOWN: usize = 65;
    pub const COMMENT: usize = 66;
    pub const NODE_STRING: usize = 67;
    pub const MINUS: usize = 68;
    pub const UNICODE_TEXT: usize = 69;
    pub const PIPE: usize = 70;
    pub const PE: usize = 71;
    pub const PS: usize = 72;
    pub const SQE: usize = 73;
    pub const SQS: usize = 74;
    pub const DIAMOND_STOP: usize = 75;
    pub const DIAMOND_START: usize = 76;
    pub const NEWLINE: usize = 77;
    pub const SPACE: usize = 78;
    pub const EOF: usize = 79;
}

pub const TOKEN_NAMES: [&str; 80] = [
    "$end",
    "INVALID",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "SHAPE_DATA",
    "CALLBACKNAME",
    "CALLBACKARGS",
    "MD_STR",
    "STR",
    "STYLE",
    "DEFAULT",
    "LINKSTYLE",
    "INTERPOLATE",
    "CLASSDEF",
    "CLASS",
    "HREF",
    "CLICK",
    "GRAPH",
    "flow",
    "connector",
    "global",
    "end",
    "LINK_TARGET",
    "NODIR",
    "DIR",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "direction_td",
    "LINK_ID",
    "NUM",
    "BRKT",
    "STYLE_SEPARATOR",
    "COLON",
    "AMP",
    "SEMI",
    "COMMA",
    "MULT",
    "LINK",
    "START_LINK",
    "EDGE_TEXT",
    "-)",
    "TEXT",
    "(-",
    "STADIUMEND",
    "STADIUMSTART",
    "SUBROUTINEEND",
    "SUBROUTINESTART",
    "VERTEX_WITH_PROPS_START",
    "TAGEND",
    "CYLINDEREND",
    "CYLINDERSTART",
    "DOUBLECIRCLEEND",
    "DOUBLECIRCLESTART",
    "TRAPEND",
    "INVTRAPEND",
    "TRAPSTART",
    "INVTRAPSTART",
    "TAGSTART",
    "UP",
    "SEP",
    "DOWN",
    "COMMENT",
    "NODE_STRING",
    "MINUS",
    "UNICODE_TEXT",
    "PIPE",
    "PE",
    "PS",
    "SQE",
    "SQS",
    "DIAMOND_STOP",
    "DIAMOND_START",
    "NEWLINE",
    "SPACE",
    "EOF",
];
