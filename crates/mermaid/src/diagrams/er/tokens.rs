use crate::token::grammar;

grammar! {
    /// The ER lexer's token kinds.
    pub enum T {
        End = "$end",
        Error = "INVALID",
        ErDiagram = "ER_DIAGRAM",
        Eof = "EOF",
        Newline = "NEWLINE",
        Colon = "COLON",
        StyleSeparator = "STYLE_SEPARATOR",
        BlockStart = "BLOCK_START",
        BlockStop = "BLOCK_STOP",
        Sqs = "SQS",
        Sqe = "SQE",
        AccTitle = "acc_title",
        AccTitleValue = "acc_title_value",
        AccDescr = "acc_descr",
        AccDescrValue = "acc_descr_value",
        AccDescrMultilineValue = "acc_descr_multiline_value",
        EndKw = "END",
        Subgraph = "SUBGRAPH",
        DirectionTb = "direction_tb",
        DirectionBt = "direction_bt",
        DirectionRl = "direction_rl",
        DirectionLr = "direction_lr",
        Classdef = "CLASSDEF",
        UnicodeText = "UNICODE_TEXT",
        StyleText = "STYLE_TEXT",
        Comma = "COMMA",
        Class = "CLASS",
        Style = "STYLE",
        Semi = "SEMI",
        Num = "NUM",
        Brkt = "BRKT",
        EntityName = "ENTITY_NAME",
        DecimalNum = "DECIMAL_NUM",
        EntityOne = "ENTITY_ONE",
        AttributeWord = "ATTRIBUTE_WORD",
        AttributeKey = "ATTRIBUTE_KEY",
        Comment = "COMMENT",
        ZeroOrOne = "ZERO_OR_ONE",
        ZeroOrMore = "ZERO_OR_MORE",
        OneOrMore = "ONE_OR_MORE",
        OnlyOne = "ONLY_ONE",
        MdParent = "MD_PARENT",
        NonIdentifying = "NON_IDENTIFYING",
        Identifying = "IDENTIFYING",
        Word = "WORD",
        /// A character no rule names; the grammar gives it meaning only as `?` and
        /// `,` inside a block.
        Char = "CHAR",
    }

    fn got(self, text, _previous) {
        match self {
            Self::Char => text,
            _ => self.name(),
        }
    }
}
