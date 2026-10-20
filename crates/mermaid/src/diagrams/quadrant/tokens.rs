//! Pele's `diagrams/quadrant/tokens.ts`.

use crate::{diagrams::common::chart::ChartKind, token::grammar};

grammar! {
    /// The quadrantChart lexer's token kinds.
    pub enum T {
        End = "$end",
        Error = "INVALID",
        Newline = "NEWLINE",
        Title = "title",
        TitleValue = "title_value",
        AccTitle = "acc_title",
        AccTitleValue = "acc_title_value",
        AccDescr = "acc_descr",
        AccDescrValue = "acc_descr_value",
        AccDescrMultilineValue = "acc_descr_multiline_value",
        XAxis = "X-AXIS",
        YAxis = "Y-AXIS",
        Delimiter = "AXIS-TEXT-DELIMITER",
        Quadrant1 = "QUADRANT_1",
        Quadrant2 = "QUADRANT_2",
        Quadrant3 = "QUADRANT_3",
        Quadrant4 = "QUADRANT_4",
        Classdef = "CLASSDEF",
        MdStr = "MD_STR",
        Str = "STR",
        ClassName = "class_name",
        PointStart = "point_start",
        PointX = "point_x",
        PointY = "point_y",
        Quadrant = "QUADRANT",
        Alpha = "ALPHA",
        UnicodeText = "UNICODE_TEXT",
        Colon = "COLON",
        Plus = "PLUS",
        Comma = "COMMA",
        Equals = "EQUALS",
        Mult = "MULT",
        Brkt = "BRKT",
        Underscore = "UNDERSCORE",
        Dot = "DOT",
        Amp = "AMP",
        Minus = "MINUS",
        Num = "NUM",
        Space = "SPACE",
        Semi = "SEMI",
        Punctuation = "PUNCTUATION",
        Eof = "EOF",
    }
}

/// `QUADRANT_1` to `QUADRANT_4`.
pub const QUADRANTS: [T; 4] = [T::Quadrant1, T::Quadrant2, T::Quadrant3, T::Quadrant4];

impl T {
    pub fn is_word(self) -> bool {
        matches!(
            self,
            Self::Punctuation
                | Self::Amp
                | Self::Num
                | Self::Alpha
                | Self::Comma
                | Self::Plus
                | Self::Equals
                | Self::Mult
                | Self::Dot
                | Self::Brkt
                | Self::Underscore
                | Self::UnicodeText
        )
    }

    pub fn is_text(self) -> bool {
        matches!(self, Self::Str | Self::MdStr) || self.is_word()
    }

    pub fn is_id(self) -> bool {
        matches!(
            self,
            Self::Alpha
                | Self::Num
                | Self::Minus
                | Self::Comma
                | Self::Colon
                | Self::Amp
                | Self::Brkt
                | Self::Mult
                | Self::UnicodeText
        )
    }

    pub fn is_style(self) -> bool {
        matches!(
            self,
            Self::Alpha | Self::Num | Self::Colon | Self::Space | Self::Brkt | Self::Minus
        )
    }
}

impl ChartKind for T {
    const NEWLINE: Self = Self::Newline;
    const ACC_TITLE: Self = Self::AccTitle;
    const ACC_TITLE_VALUE: Self = Self::AccTitleValue;
    const ACC_DESCR: Self = Self::AccDescr;
    const ACC_DESCR_VALUE: Self = Self::AccDescrValue;
    const ACC_DESCR_MULTILINE_VALUE: Self = Self::AccDescrMultilineValue;
    const STR: Self = Self::Str;
    const ALPHA: Self = Self::Alpha;
    const NUM: Self = Self::Num;
    const COLON: Self = Self::Colon;
    const PLUS: Self = Self::Plus;
    const COMMA: Self = Self::Comma;
    const EQUALS: Self = Self::Equals;
    const MULT: Self = Self::Mult;
    const BRKT: Self = Self::Brkt;
    const UNDERSCORE: Self = Self::Underscore;
    const DOT: Self = Self::Dot;
    const AMP: Self = Self::Amp;
    const MINUS: Self = Self::Minus;
    const SEMI: Self = Self::Semi;
    const EOF: Self = Self::Eof;
}
