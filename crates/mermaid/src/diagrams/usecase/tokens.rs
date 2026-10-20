//! Mermaid's token types, in an order that makes its token categories
//! ranges: WORD is `Usecase` to `Identifier`, and LABEL_TEXT is `Usecase` to
//! `LabelSymbol`. Pele's `diagrams/usecase/tokens.ts`.

/// The use case lexer's token types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum T {
    Eof,
    Usecase,
    Actor,
    SystemBoundary,
    End,
    Direction,
    Td,
    Tb,
    Bt,
    Lr,
    Rl,
    Note,
    For,
    Json,
    ClassDef,
    Class,
    Style,
    Include,
    Extend,
    True,
    False,
    Identifier,
    Number,
    At,
    Comma,
    Colon,
    HashColor,
    CssIdentifier,
    CssEscapedComma,
    Dash,
    Dot,
    Percent,
    CssPunctuation,
    LabelPunctuation,
    LabelSymbol,
    PlainString,
    MarkdownString,
    UnclosedMarkdownString,
    Comment,
    Newline,
    AccDescrBlock,
    AccTitleLine,
    AccDescrLine,
    JsonDeclarationStart,
    JsonObjectLiteral,
    UnclosedJsonObjectLiteral,
    Generalization,
    DependencyArrow,
    StereotypeStart,
    StereotypeEnd,
    StereotypeText,
    UnclosedStereotypeText,
    ClassSeparator,
    ForwardSolid,
    BackwardSolid,
    ForwardCircle,
    BackwardCircle,
    ForwardCross,
    BackwardCross,
    MarkerlessSolid,
    MetadataStart,
    Lbrace,
    Rbrace,
    Lbracket,
    Rbracket,
    Lparen,
    Rparen,
}

impl T {
    /// Mermaid's name for the token type.
    pub fn name(self) -> &'static str {
        match self {
            Self::Eof => "EOF",
            Self::Usecase => "USECASE",
            Self::Actor => "ACTOR",
            Self::SystemBoundary => "SYSTEM_BOUNDARY",
            Self::End => "END",
            Self::Direction => "DIRECTION",
            Self::Td => "TD",
            Self::Tb => "TB",
            Self::Bt => "BT",
            Self::Lr => "LR",
            Self::Rl => "RL",
            Self::Note => "NOTE",
            Self::For => "FOR",
            Self::Json => "JSON",
            Self::ClassDef => "CLASS_DEF",
            Self::Class => "CLASS",
            Self::Style => "STYLE",
            Self::Include => "INCLUDE",
            Self::Extend => "EXTEND",
            Self::True => "TRUE",
            Self::False => "FALSE",
            Self::Identifier => "IDENTIFIER",
            Self::Number => "NUMBER",
            Self::At => "AT",
            Self::Comma => "COMMA",
            Self::Colon => "COLON",
            Self::HashColor => "HASH_COLOR",
            Self::CssIdentifier => "CSS_IDENTIFIER",
            Self::CssEscapedComma => "CSS_ESCAPED_COMMA",
            Self::Dash => "DASH",
            Self::Dot => "DOT",
            Self::Percent => "PERCENT",
            Self::CssPunctuation => "CSS_PUNCTUATION",
            Self::LabelPunctuation => "LABEL_PUNCTUATION",
            Self::LabelSymbol => "LABEL_SYMBOL",
            Self::PlainString => "PLAIN_STRING",
            Self::MarkdownString => "MARKDOWN_STRING",
            Self::UnclosedMarkdownString => "UNCLOSED_MARKDOWN_STRING",
            Self::Comment => "COMMENT",
            Self::Newline => "NEWLINE",
            Self::AccDescrBlock => "ACC_DESCR_BLOCK",
            Self::AccTitleLine => "ACC_TITLE_LINE",
            Self::AccDescrLine => "ACC_DESCR_LINE",
            Self::JsonDeclarationStart => "JSON_DECLARATION_START",
            Self::JsonObjectLiteral => "JSON_OBJECT_LITERAL",
            Self::UnclosedJsonObjectLiteral => "UNCLOSED_JSON_OBJECT_LITERAL",
            Self::Generalization => "GENERALIZATION",
            Self::DependencyArrow => "DEPENDENCY_ARROW",
            Self::StereotypeStart => "STEREOTYPE_START",
            Self::StereotypeEnd => "STEREOTYPE_END",
            Self::StereotypeText => "STEREOTYPE_TEXT",
            Self::UnclosedStereotypeText => "UNCLOSED_STEREOTYPE_TEXT",
            Self::ClassSeparator => "CLASS_SEPARATOR",
            Self::ForwardSolid => "FORWARD_SOLID",
            Self::BackwardSolid => "BACKWARD_SOLID",
            Self::ForwardCircle => "FORWARD_CIRCLE",
            Self::BackwardCircle => "BACKWARD_CIRCLE",
            Self::ForwardCross => "FORWARD_CROSS",
            Self::BackwardCross => "BACKWARD_CROSS",
            Self::MarkerlessSolid => "MARKERLESS_SOLID",
            Self::MetadataStart => "METADATA_START",
            Self::Lbrace => "LBRACE",
            Self::Rbrace => "RBRACE",
            Self::Lbracket => "LBRACKET",
            Self::Rbracket => "RBRACKET",
            Self::Lparen => "LPAREN",
            Self::Rparen => "RPAREN",
        }
    }

    /// The relationship operators, in Mermaid's order.
    pub const ARROWS: [T; 9] = [
        T::DependencyArrow,
        T::Generalization,
        T::ForwardSolid,
        T::BackwardSolid,
        T::MarkerlessSolid,
        T::ForwardCircle,
        T::BackwardCircle,
        T::ForwardCross,
        T::BackwardCross,
    ];
    /// What names an element, in Mermaid's order.
    pub const NAMES: [T; 3] = [T::Identifier, T::PlainString, T::MarkdownString];
    /// What starts a top-level statement other than a name, in Mermaid's
    /// order.
    pub const STATEMENTS: [T; 11] = [
        T::AccTitleLine,
        T::AccDescrLine,
        T::AccDescrBlock,
        T::Direction,
        T::Actor,
        T::SystemBoundary,
        T::Note,
        T::JsonDeclarationStart,
        T::ClassDef,
        T::Class,
        T::Style,
    ];
    /// What a style value is made of besides [`T::is_word`], in Mermaid's
    /// order.
    pub const STYLE_PARTS: [T; 18] = [
        T::PlainString,
        T::Number,
        T::HashColor,
        T::CssIdentifier,
        T::CssEscapedComma,
        T::Dash,
        T::Dot,
        T::Percent,
        T::CssPunctuation,
        T::Colon,
        T::Lparen,
        T::Rparen,
        T::Lbracket,
        T::Rbracket,
        T::Lbrace,
        T::Rbrace,
        T::At,
        T::MarkerlessSolid,
    ];
    /// What may follow `name@{`, in Mermaid's order.
    pub const METADATA_FIRST: [T; 4] = [T::Newline, T::Identifier, T::PlainString, T::Rbrace];
    /// Mermaid's name for the [`T::is_word`] category.
    pub const WORD: &str = "WORD";
    /// Mermaid's name for the [`T::is_label_text`] category.
    pub const LABEL_TEXT: &str = "LABEL_TEXT";

    /// Mermaid's WORD category: every keyword and IDENTIFIER.
    pub fn is_word(self) -> bool {
        (T::Usecase..=T::Identifier).contains(&self)
    }

    /// Mermaid's LABEL_TEXT category.
    pub fn is_label_text(self) -> bool {
        (T::Usecase..=T::LabelSymbol).contains(&self)
    }

    /// LABEL_TEXT or a string.
    pub fn is_label_part(self) -> bool {
        (T::Usecase..=T::MarkdownString).contains(&self)
    }

    pub fn is_arrow(self) -> bool {
        Self::ARROWS.contains(&self)
    }

    pub fn is_name(self) -> bool {
        Self::NAMES.contains(&self)
    }

    /// A metadata key, or a value other than a boolean.
    pub fn is_key(self) -> bool {
        matches!(self, T::Identifier | T::PlainString)
    }

    pub fn is_style_part(self) -> bool {
        self.is_word() || Self::STYLE_PARTS.contains(&self)
    }
}
