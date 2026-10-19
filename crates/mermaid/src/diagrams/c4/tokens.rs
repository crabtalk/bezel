//! Pele's `diagrams/c4/tokens.ts`.

pub const END: u8 = 0;
pub const ERROR: u8 = 1;
pub const DIRECTION_TB: u8 = 2;
pub const DIRECTION_BT: u8 = 3;
pub const DIRECTION_RL: u8 = 4;
pub const DIRECTION_LR: u8 = 5;
pub const TITLE: u8 = 6;
pub const ACC_DESCRIPTION: u8 = 7;
pub const ACC_TITLE: u8 = 8;
pub const ACC_TITLE_VALUE: u8 = 9;
pub const ACC_DESCR: u8 = 10;
pub const ACC_DESCR_VALUE: u8 = 11;
pub const ACC_DESCR_MULTILINE_VALUE: u8 = 12;
pub const NEWLINE: u8 = 13;
pub const C4_CONTEXT: u8 = 14;
pub const C4_CONTAINER: u8 = 15;
pub const C4_COMPONENT: u8 = 16;
pub const C4_DYNAMIC: u8 = 17;
pub const C4_DEPLOYMENT: u8 = 18;
pub const PERSON: u8 = 19;
pub const PERSON_EXT: u8 = 20;
pub const SYSTEM: u8 = 21;
pub const SYSTEM_DB: u8 = 22;
pub const SYSTEM_QUEUE: u8 = 23;
pub const SYSTEM_EXT: u8 = 24;
pub const SYSTEM_EXT_DB: u8 = 25;
pub const SYSTEM_EXT_QUEUE: u8 = 26;
pub const CONTAINER: u8 = 27;
pub const CONTAINER_DB: u8 = 28;
pub const CONTAINER_QUEUE: u8 = 29;
pub const CONTAINER_EXT: u8 = 30;
pub const CONTAINER_EXT_DB: u8 = 31;
pub const CONTAINER_EXT_QUEUE: u8 = 32;
pub const COMPONENT: u8 = 33;
pub const COMPONENT_DB: u8 = 34;
pub const COMPONENT_QUEUE: u8 = 35;
pub const COMPONENT_EXT: u8 = 36;
pub const COMPONENT_EXT_DB: u8 = 37;
pub const COMPONENT_EXT_QUEUE: u8 = 38;
pub const ENTERPRISE_BOUNDARY: u8 = 39;
pub const SYSTEM_BOUNDARY: u8 = 40;
pub const BOUNDARY: u8 = 41;
pub const CONTAINER_BOUNDARY: u8 = 42;
pub const NODE: u8 = 43;
pub const NODE_L: u8 = 44;
pub const NODE_R: u8 = 45;
pub const REL: u8 = 46;
pub const BIREL: u8 = 47;
pub const REL_U: u8 = 48;
pub const REL_D: u8 = 49;
pub const REL_L: u8 = 50;
pub const REL_R: u8 = 51;
pub const REL_B: u8 = 52;
pub const REL_INDEX: u8 = 53;
pub const UPDATE_EL_STYLE: u8 = 54;
pub const UPDATE_REL_STYLE: u8 = 55;
pub const UPDATE_LAYOUT_CONFIG: u8 = 56;
pub const EOF_IN_STRUCT: u8 = 57;
pub const ATTRIBUTE_EMPTY: u8 = 58;
pub const STR: u8 = 59;
pub const STR_KEY: u8 = 60;
pub const STR_VALUE: u8 = 61;
pub const LBRACE: u8 = 62;
pub const RBRACE: u8 = 63;
pub const EOF: u8 = 64;

pub const TOKEN_NAMES: [&str; 65] = [
    "$end",
    "INVALID",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "title",
    "accDescription",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "NEWLINE",
    "C4_CONTEXT",
    "C4_CONTAINER",
    "C4_COMPONENT",
    "C4_DYNAMIC",
    "C4_DEPLOYMENT",
    "PERSON",
    "PERSON_EXT",
    "SYSTEM",
    "SYSTEM_DB",
    "SYSTEM_QUEUE",
    "SYSTEM_EXT",
    "SYSTEM_EXT_DB",
    "SYSTEM_EXT_QUEUE",
    "CONTAINER",
    "CONTAINER_DB",
    "CONTAINER_QUEUE",
    "CONTAINER_EXT",
    "CONTAINER_EXT_DB",
    "CONTAINER_EXT_QUEUE",
    "COMPONENT",
    "COMPONENT_DB",
    "COMPONENT_QUEUE",
    "COMPONENT_EXT",
    "COMPONENT_EXT_DB",
    "COMPONENT_EXT_QUEUE",
    "ENTERPRISE_BOUNDARY",
    "SYSTEM_BOUNDARY",
    "BOUNDARY",
    "CONTAINER_BOUNDARY",
    "NODE",
    "NODE_L",
    "NODE_R",
    "REL",
    "BIREL",
    "REL_U",
    "REL_D",
    "REL_L",
    "REL_R",
    "REL_B",
    "REL_INDEX",
    "UPDATE_EL_STYLE",
    "UPDATE_REL_STYLE",
    "UPDATE_LAYOUT_CONFIG",
    "EOF_IN_STRUCT",
    "ATTRIBUTE_EMPTY",
    "STR",
    "STR_KEY",
    "STR_VALUE",
    "LBRACE",
    "RBRACE",
    "EOF",
];
