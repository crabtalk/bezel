//! Pele's `diagrams/sequence/tokens.ts`.

/// The end of input, `T.END` in Pele.
pub const EOF: u8 = 0;
pub const ERROR: u8 = 1;
pub const INVALID: u8 = 2;
pub const NEWLINE: u8 = 3;
pub const SD: u8 = 4;
pub const CREATE: u8 = 5;
pub const BOX: u8 = 6;
pub const REST_OF_LINE: u8 = 7;
pub const END: u8 = 8;
pub const AUTONUMBER: u8 = 9;
pub const NUM: u8 = 10;
pub const OFF: u8 = 11;
pub const ACTIVATE: u8 = 12;
pub const DEACTIVATE: u8 = 13;
pub const TITLE: u8 = 14;
pub const LEGACY_TITLE: u8 = 15;
pub const ACC_TITLE: u8 = 16;
pub const ACC_TITLE_VALUE: u8 = 17;
pub const ACC_DESCR: u8 = 18;
pub const ACC_DESCR_VALUE: u8 = 19;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 20;
pub const LOOP: u8 = 21;
pub const RECT: u8 = 22;
pub const OPT: u8 = 23;
pub const ALT: u8 = 24;
pub const PAR: u8 = 25;
pub const PAR_OVER: u8 = 26;
pub const CRITICAL: u8 = 27;
pub const BREAK: u8 = 28;
pub const OPTION: u8 = 29;
pub const AND: u8 = 30;
pub const ELSE: u8 = 31;
pub const PARTICIPANT: u8 = 32;
pub const AS: u8 = 33;
pub const PARTICIPANT_ACTOR: u8 = 34;
pub const DESTROY: u8 = 35;
pub const NOTE: u8 = 36;
pub const OVER: u8 = 37;
pub const LINKS: u8 = 38;
pub const LINK: u8 = 39;
pub const PROPERTIES: u8 = 40;
pub const DETAILS: u8 = 41;
pub const COMMA: u8 = 42;
pub const LEFT_OF: u8 = 43;
pub const RIGHT_OF: u8 = 44;
pub const PLUS: u8 = 45;
pub const MINUS: u8 = 46;
pub const CENTRAL: u8 = 47;
pub const ACTOR: u8 = 48;
pub const CONFIG_START: u8 = 49;
pub const CONFIG_CONTENT: u8 = 50;
pub const CONFIG_END: u8 = 51;
pub const TXT: u8 = 52;
pub const SOLID_OPEN_ARROW: u8 = 53;
pub const DOTTED_OPEN_ARROW: u8 = 54;
pub const SOLID_ARROW: u8 = 55;
pub const BIDIRECTIONAL_SOLID_ARROW: u8 = 56;
pub const DOTTED_ARROW: u8 = 57;
pub const BIDIRECTIONAL_DOTTED_ARROW: u8 = 58;
pub const SOLID_CROSS: u8 = 59;
pub const DOTTED_CROSS: u8 = 60;
pub const SOLID_POINT: u8 = 61;
pub const DOTTED_POINT: u8 = 62;
pub const SOLID_ARROW_TOP: u8 = 63;
pub const SOLID_ARROW_BOTTOM: u8 = 64;
pub const STICK_ARROW_TOP: u8 = 65;
pub const STICK_ARROW_BOTTOM: u8 = 66;
pub const SOLID_ARROW_TOP_DOTTED: u8 = 67;
pub const SOLID_ARROW_BOTTOM_DOTTED: u8 = 68;
pub const STICK_ARROW_TOP_DOTTED: u8 = 69;
pub const STICK_ARROW_BOTTOM_DOTTED: u8 = 70;
pub const SOLID_ARROW_TOP_REVERSE: u8 = 71;
pub const SOLID_ARROW_BOTTOM_REVERSE: u8 = 72;
pub const STICK_ARROW_TOP_REVERSE: u8 = 73;
pub const STICK_ARROW_BOTTOM_REVERSE: u8 = 74;
pub const SOLID_ARROW_TOP_REVERSE_DOTTED: u8 = 75;
pub const SOLID_ARROW_BOTTOM_REVERSE_DOTTED: u8 = 76;
pub const STICK_ARROW_TOP_REVERSE_DOTTED: u8 = 77;
pub const STICK_ARROW_BOTTOM_REVERSE_DOTTED: u8 = 78;

/// Names as Mermaid's grammar spells them, for error messages and for
/// comparison with its parser.
pub const TOKEN_NAMES: [&str; 79] = [
    "$end",
    "INVALID",
    "INVALID",
    "NEWLINE",
    "SD",
    "create",
    "box",
    "restOfLine",
    "end",
    "autonumber",
    "NUM",
    "off",
    "activate",
    "deactivate",
    "title",
    "legacy_title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "loop",
    "rect",
    "opt",
    "alt",
    "par",
    "par_over",
    "critical",
    "break",
    "option",
    "and",
    "else",
    "participant",
    "AS",
    "participant_actor",
    "destroy",
    "note",
    "over",
    "links",
    "link",
    "properties",
    "details",
    ",",
    "left_of",
    "right_of",
    "+",
    "-",
    "()",
    "ACTOR",
    "CONFIG_START",
    "CONFIG_CONTENT",
    "CONFIG_END",
    "TXT",
    "SOLID_OPEN_ARROW",
    "DOTTED_OPEN_ARROW",
    "SOLID_ARROW",
    "BIDIRECTIONAL_SOLID_ARROW",
    "DOTTED_ARROW",
    "BIDIRECTIONAL_DOTTED_ARROW",
    "SOLID_CROSS",
    "DOTTED_CROSS",
    "SOLID_POINT",
    "DOTTED_POINT",
    "SOLID_ARROW_TOP",
    "SOLID_ARROW_BOTTOM",
    "STICK_ARROW_TOP",
    "STICK_ARROW_BOTTOM",
    "SOLID_ARROW_TOP_DOTTED",
    "SOLID_ARROW_BOTTOM_DOTTED",
    "STICK_ARROW_TOP_DOTTED",
    "STICK_ARROW_BOTTOM_DOTTED",
    "SOLID_ARROW_TOP_REVERSE",
    "SOLID_ARROW_BOTTOM_REVERSE",
    "STICK_ARROW_TOP_REVERSE",
    "STICK_ARROW_BOTTOM_REVERSE",
    "SOLID_ARROW_TOP_REVERSE_DOTTED",
    "SOLID_ARROW_BOTTOM_REVERSE_DOTTED",
    "STICK_ARROW_TOP_REVERSE_DOTTED",
    "STICK_ARROW_BOTTOM_REVERSE_DOTTED",
];
