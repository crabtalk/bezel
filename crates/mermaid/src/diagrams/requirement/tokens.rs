//! Pele's `diagrams/requirement/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const TITLE: u8 = 2;
pub const ACC_TITLE: u8 = 3;
pub const ACC_TITLE_VALUE: u8 = 4;
pub const ACC_DESCR: u8 = 5;
pub const ACC_DESCR_VALUE: u8 = 6;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 7;
pub const DIRECTION_TB: u8 = 8;
pub const DIRECTION_BT: u8 = 9;
pub const DIRECTION_RL: u8 = 10;
pub const DIRECTION_LR: u8 = 11;
pub const NEWLINE: u8 = 12;
pub const EOF: u8 = 13;
pub const RD: u8 = 14;
pub const STRUCT_START: u8 = 15;
pub const STRUCT_STOP: u8 = 16;
pub const STYLE_SEPARATOR: u8 = 17;
pub const COLONSEP: u8 = 18;
pub const ID: u8 = 19;
pub const TEXT: u8 = 20;
pub const RISK: u8 = 21;
pub const VERIFYMTHD: u8 = 22;
pub const REQUIREMENT: u8 = 23;
pub const FUNCTIONAL_REQUIREMENT: u8 = 24;
pub const INTERFACE_REQUIREMENT: u8 = 25;
pub const PERFORMANCE_REQUIREMENT: u8 = 26;
pub const PHYSICAL_REQUIREMENT: u8 = 27;
pub const DESIGN_CONSTRAINT: u8 = 28;
pub const LOW_RISK: u8 = 29;
pub const MED_RISK: u8 = 30;
pub const HIGH_RISK: u8 = 31;
pub const VERIFY_ANALYSIS: u8 = 32;
pub const VERIFY_DEMONSTRATION: u8 = 33;
pub const VERIFY_INSPECTION: u8 = 34;
pub const VERIFY_TEST: u8 = 35;
pub const ELEMENT: u8 = 36;
pub const CONTAINS: u8 = 37;
pub const COPIES: u8 = 38;
pub const DERIVES: u8 = 39;
pub const SATISFIES: u8 = 40;
pub const VERIFIES: u8 = 41;
pub const REFINES: u8 = 42;
pub const TRACES: u8 = 43;
pub const TYPE: u8 = 44;
pub const DOCREF: u8 = 45;
pub const STYLE: u8 = 46;
pub const CLASSDEF: u8 = 47;
pub const CLASS: u8 = 48;
pub const ALPHA: u8 = 49;
pub const COLON: u8 = 50;
pub const SEMICOLON: u8 = 51;
pub const PERCENT: u8 = 52;
pub const MINUS: u8 = 53;
pub const BRKT: u8 = 54;
pub const END_ARROW_L: u8 = 55;
pub const END_ARROW_R: u8 = 56;
pub const LINE: u8 = 57;
pub const QSTRING: u8 = 58;
pub const UNQSTRING: u8 = 59;
pub const COMMA: u8 = 60;

pub const TOKEN_NAMES: [&str; 61] = [
    "$end",
    "INVALID",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "NEWLINE",
    "EOF",
    "RD",
    "STRUCT_START",
    "STRUCT_STOP",
    "STYLE_SEPARATOR",
    "COLONSEP",
    "ID",
    "TEXT",
    "RISK",
    "VERIFYMTHD",
    "REQUIREMENT",
    "FUNCTIONAL_REQUIREMENT",
    "INTERFACE_REQUIREMENT",
    "PERFORMANCE_REQUIREMENT",
    "PHYSICAL_REQUIREMENT",
    "DESIGN_CONSTRAINT",
    "LOW_RISK",
    "MED_RISK",
    "HIGH_RISK",
    "VERIFY_ANALYSIS",
    "VERIFY_DEMONSTRATION",
    "VERIFY_INSPECTION",
    "VERIFY_TEST",
    "ELEMENT",
    "CONTAINS",
    "COPIES",
    "DERIVES",
    "SATISFIES",
    "VERIFIES",
    "REFINES",
    "TRACES",
    "TYPE",
    "DOCREF",
    "STYLE",
    "CLASSDEF",
    "CLASS",
    "ALPHA",
    "COLON",
    "SEMICOLON",
    "PERCENT",
    "MINUS",
    "BRKT",
    "END_ARROW_L",
    "END_ARROW_R",
    "LINE",
    "qString",
    "unqString",
    "COMMA",
];

/// Every keyword is matched without regard to case and must end at a word
/// boundary.
pub const KEYWORDS: [(&str, u8); 31] = [
    ("requirementdiagram", RD),
    ("id", ID),
    ("text", TEXT),
    ("risk", RISK),
    ("verifymethod", VERIFYMTHD),
    ("requirement", REQUIREMENT),
    ("functionalrequirement", FUNCTIONAL_REQUIREMENT),
    ("interfacerequirement", INTERFACE_REQUIREMENT),
    ("performancerequirement", PERFORMANCE_REQUIREMENT),
    ("physicalrequirement", PHYSICAL_REQUIREMENT),
    ("designconstraint", DESIGN_CONSTRAINT),
    ("low", LOW_RISK),
    ("medium", MED_RISK),
    ("high", HIGH_RISK),
    ("analysis", VERIFY_ANALYSIS),
    ("demonstration", VERIFY_DEMONSTRATION),
    ("inspection", VERIFY_INSPECTION),
    ("test", VERIFY_TEST),
    ("element", ELEMENT),
    ("contains", CONTAINS),
    ("copies", COPIES),
    ("derives", DERIVES),
    ("satisfies", SATISFIES),
    ("verifies", VERIFIES),
    ("refines", REFINES),
    ("traces", TRACES),
    ("type", TYPE),
    ("docref", DOCREF),
    ("style", STYLE),
    ("classdef", CLASSDEF),
    ("class", CLASS),
];
