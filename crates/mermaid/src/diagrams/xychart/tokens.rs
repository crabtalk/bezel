//! Pele's `diagrams/xychart/tokens.ts`.

use crate::{diagrams::common::chart::ChartKind, token::grammar};

grammar! {
    /// The xychart lexer's token kinds.
    pub enum T {
        End = "$end",
        Error = "INVALID",
        Newline = "NEWLINE",
        Title = "title",
        AccTitle = "acc_title",
        AccTitleValue = "acc_title_value",
        AccDescr = "acc_descr",
        AccDescrValue = "acc_descr_value",
        AccDescrMultilineValue = "acc_descr_multiline_value",
        Xychart = "XYCHART",
        ChartOrientation = "CHART_ORIENTATION",
        XAxis = "X_AXIS",
        YAxis = "Y_AXIS",
        ArrowDelimiter = "ARROW_DELIMITER",
        Line = "LINE",
        Bar = "BAR",
        NumberWithDecimal = "NUMBER_WITH_DECIMAL",
        SquareBracesStart = "SQUARE_BRACES_START",
        SquareBracesEnd = "SQUARE_BRACES_END",
        Str = "STR",
        Alpha = "ALPHA",
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
        Semi = "SEMI",
        Eof = "EOF",
    }
}

impl T {
    pub fn is_word(self) -> bool {
        matches!(
            self,
            Self::Amp
                | Self::Num
                | Self::Alpha
                | Self::Plus
                | Self::Equals
                | Self::Mult
                | Self::Dot
                | Self::Brkt
                | Self::Minus
                | Self::Underscore
        )
    }

    pub fn is_text(self) -> bool {
        self == Self::Str || self.is_word()
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
