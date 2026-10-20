//! Pele's `diagrams/eventmodeling/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Header, Reader, TokenType, acc_title, directive, keyword, single_line_comment, title,
            token_kinds, yaml,
        },
    },
    error::Error,
};

token_kinds! {
    enum Token {
        Keyword = keyword("eventmodeling").longer(&[Token::Id]),
        ResetFrame = word("resetframe"),
        ReadModel = word("readmodel"),
        Processor = word("processor"),
        TimeFrame = word("timeframe"),
        Command = word("command"),
        Entity = word("entity"),
        Event = word("event"),
        Jsobj = word("jsobj"),
        Figma = word("figma"),
        Given = word("given"),
        Json = word("json"),
        Salt = word("salt"),
        Html = word("html"),
        Text = word("text"),
        Data = word("data"),
        Note = word("note"),
        When = word("when"),
        Then = word("then"),
        Rmo = word("rmo"),
        Cmd = word("cmd"),
        Evt = word("evt"),
        Pcr = word("pcr"),
        Uri = word("uri"),
        Arrow = TokenType::literal("->>"),
        Gwt = word("gwt"),
        Ui = word("ui"),
        Md = word("md"),
        Tf = word("tf"),
        Open = TokenType::literal("[["),
        Close = TokenType::literal("]]"),
        Rf = word("rf"),
        Tick = TokenType::literal("`"),
        Dot = TokenType::literal("."),
        Id = TokenType::regex("EM_ID", r"[_a-zA-Z][\w_]*", None),
        FrameId = TokenType::regex("EM_FID", r"\d{1,3}", None),
        Inline = TokenType::regex("EM_DATA_INLINE", r#"\{(.*)\}|"(.*)"|'(.*)'"#, None),
        Block = TokenType::regex(
            "EM_DATA_BLOCK",
            r"\{[\t ]*\r?\n(?:[\S\s]*?\r?\n)?\}(?:\r?\n|(?!\S))",
            None,
        ),
        AccDescr = acc_descr("EM_ACC_DESCR"),
        AccTitle = acc_title().named("EM_ACC_TITLE"),
        Title = title().named("EM_TITLE"),
        Ws = TokenType::regex("EM_WS", r"\s+", None).hidden(),
        Yaml = yaml().named("EM_YAML"),
        Directive = directive().named("EM_DIRECTIVE"),
        Comment = single_line_comment().named("EM_SINGLE_LINE_COMMENT"),
        MlComment = TokenType::regex("EM_ML_COMMENT", r"\/\*[\s\S]*?\*\/", None).hidden(),
        SlComment = TokenType::regex("EM_SL_COMMENT", r"\/\/[^\n\r]*", None).hidden(),
    }
}

/// Langium gives every keyword that is also an identifier the identifier as
/// a longer alternative.
fn word(text: &'static str) -> TokenType<Token> {
    TokenType::literal(text).longer(&[Token::Id])
}

/// A frame's model entity type, by its name or short name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Ui,
    Processor,
    ReadModel,
    Command,
    Event,
}

impl Token {
    fn entity_type(self) -> Option<EntityType> {
        Some(match self {
            Self::Rmo | Self::ReadModel => EntityType::ReadModel,
            Self::Ui => EntityType::Ui,
            Self::Cmd | Self::Command => EntityType::Command,
            Self::Evt | Self::Event => EntityType::Event,
            Self::Pcr | Self::Processor => EntityType::Processor,
            _ => return None,
        })
    }

    fn is_data_type(self) -> bool {
        matches!(
            self,
            Self::Json
                | Self::Jsobj
                | Self::Figma
                | Self::Salt
                | Self::Uri
                | Self::Md
                | Self::Html
                | Self::Text
        )
    }
}

pub struct EmFrame {
    pub reset: bool,
    pub name: String,
    pub model_entity_type: EntityType,
    pub entity_identifier: String,
    pub source_frames: Vec<String>,
    pub data_reference: Option<String>,
    pub data_type: Option<String>,
    pub data_inline_value: Option<String>,
    /// Offset of the frame in the source, for error messages.
    pub offset: usize,
}

pub struct EmDataEntity {
    pub name: String,
    pub data_type: Option<String>,
    pub data_block_value: String,
}

pub struct EmNoteEntity {
    pub source_frame: String,
    pub data_type: Option<String>,
    pub data_block_value: String,
}

pub struct EmGwt {
    pub source_frame: String,
    pub given_statements: Vec<String>,
    pub when_statements: Vec<String>,
    pub then_statements: Vec<String>,
}

#[derive(Default)]
pub struct EventModelAst {
    pub header: Header,
    pub model_entities: Vec<String>,
    pub frames: Vec<EmFrame>,
    pub data_entities: Vec<EmDataEntity>,
    pub note_entities: Vec<EmNoteEntity>,
    pub gwt_entities: Vec<EmGwt>,
    pub source: String,
}

struct Parser<'s> {
    r: Reader<'s, Token>,
}

impl Parser<'_> {
    fn entity_type(&mut self) -> Result<EntityType, Error> {
        let Some(entity_type) = self.r.kind().and_then(Token::entity_type) else {
            return Err(self.r.fail("an entity type"));
        };
        self.r.bump();
        Ok(entity_type)
    }

    fn qualified_name(&mut self) -> Result<String, Error> {
        let mut name = self.r.expect(Token::Id)?.to_string();
        while self.r.accept(Token::Dot) {
            name.push('.');
            name.push_str(self.r.expect(Token::Id)?);
        }
        Ok(name)
    }

    fn data_type(&mut self) -> Result<Option<String>, Error> {
        if !self.r.accept(Token::Tick) {
            return Ok(None);
        }
        if !self.r.kind().is_some_and(Token::is_data_type) {
            return Err(self.r.fail("a data type"));
        }
        let kind = self.r.take().to_string();
        self.r.expect(Token::Tick)?;
        Ok(Some(kind))
    }

    fn statements(&mut self) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        loop {
            self.entity_type()?;
            out.push(self.r.expect(Token::Id)?.to_string());
            if self.r.kind().and_then(Token::entity_type).is_none() {
                break;
            }
        }
        Ok(out)
    }

    /// A frame after its keyword at `offset`.
    fn frame(&mut self, reset: bool, offset: usize) -> Result<EmFrame, Error> {
        let name = self.r.expect(Token::FrameId)?.to_string();
        let model_entity_type = self.entity_type()?;
        let entity_identifier = self.qualified_name()?;
        let mut frame = EmFrame {
            reset,
            name,
            model_entity_type,
            entity_identifier,
            source_frames: Vec::new(),
            data_reference: None,
            data_type: None,
            data_inline_value: None,
            offset,
        };
        while self.r.accept(Token::Arrow) {
            frame
                .source_frames
                .push(self.r.expect(Token::FrameId)?.to_string());
        }
        if self.r.accept(Token::Open) {
            frame.data_reference = Some(self.r.expect(Token::Id)?.to_string());
            self.r.expect(Token::Close)?;
        }
        if self.r.at(Token::Tick) || self.r.at(Token::Inline) {
            if let Some(kind) = self.data_type()? {
                frame.data_type = Some(kind);
            }
            frame.data_inline_value = Some(self.r.expect(Token::Inline)?.to_string());
        }
        Ok(frame)
    }

    fn gwt(&mut self) -> Result<EmGwt, Error> {
        let source_frame = self.r.expect(Token::FrameId)?.to_string();
        self.r.expect(Token::Given)?;
        let given_statements = self.statements()?;
        let when_statements = if self.r.accept(Token::When) {
            self.statements()?
        } else {
            Vec::new()
        };
        self.r.expect(Token::Then)?;
        let then_statements = self.statements()?;
        Ok(EmGwt {
            source_frame,
            given_statements,
            when_statements,
            then_statements,
        })
    }
}

pub fn parse_event_model(src: &str) -> Result<EventModelAst, Error> {
    let mut p = Parser {
        r: Reader::new(src, Kind::Eventmodeling)?,
    };
    let mut ast = EventModelAst {
        source: src.to_string(),
        ..EventModelAst::default()
    };

    p.r.expect(Token::Keyword)?;
    while let Some(kind) = p.r.kind() {
        if p.r.heading(&mut ast.header) {
            continue;
        }
        let offset = p.r.start();
        match kind {
            Token::Entity => {
                p.r.bump();
                ast.model_entities.push(p.qualified_name()?);
            }
            Token::Tf | Token::TimeFrame | Token::Rf | Token::ResetFrame => {
                p.r.bump();
                let reset = matches!(kind, Token::Rf | Token::ResetFrame);
                ast.frames.push(p.frame(reset, offset)?);
            }
            Token::Data => {
                p.r.bump();
                let name = p.r.expect(Token::Id)?.to_string();
                let data_type = p.data_type()?;
                let data_block_value = p.r.expect(Token::Block)?.to_string();
                ast.data_entities.push(EmDataEntity {
                    name,
                    data_type,
                    data_block_value,
                });
            }
            Token::Note => {
                p.r.bump();
                let source_frame = p.r.expect(Token::FrameId)?.to_string();
                let data_type = p.data_type()?;
                let data_block_value = p.r.expect(Token::Block)?.to_string();
                ast.note_entities.push(EmNoteEntity {
                    source_frame,
                    data_type,
                    data_block_value,
                });
            }
            Token::Gwt => {
                p.r.bump();
                ast.gwt_entities.push(p.gwt()?);
            }
            _ => {
                return Err(p
                    .r
                    .fail("a frame, an entity, a data block, a note, or a specification"));
            }
        }
    }
    Ok(ast)
}
