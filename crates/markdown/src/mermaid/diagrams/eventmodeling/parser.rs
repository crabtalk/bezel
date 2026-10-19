//! Pele's `diagrams/eventmodeling/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Reader, Table, TokenType, acc_title, directive, keyword, single_line_comment, title,
            tokenize, yaml,
        },
    },
    error::Error,
};

const WORDS: [&str; 23] = [
    "resetframe",
    "readmodel",
    "processor",
    "timeframe",
    "command",
    "entity",
    "event",
    "jsobj",
    "figma",
    "given",
    "json",
    "salt",
    "html",
    "text",
    "data",
    "note",
    "when",
    "then",
    "rmo",
    "cmd",
    "evt",
    "pcr",
    "uri",
];

pub static EVENTMODELING_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let mut types = vec![keyword("eventmodeling").longer(&["EM_ID"])];
    types.extend(WORDS.iter().map(|text| word(text)));
    types.extend([
        sign("->>"),
        word("gwt"),
        word("ui"),
        word("md"),
        word("tf"),
        sign("[["),
        sign("]]"),
        word("rf"),
        sign("`"),
        sign("."),
        TokenType::regex("EM_ID", r"[_a-zA-Z][\w_]*", None),
        TokenType::regex("EM_FID", r"\d{1,3}", None),
        TokenType::regex("EM_DATA_INLINE", r#"\{(.*)\}|"(.*)"|'(.*)'"#, None),
        TokenType::regex(
            "EM_DATA_BLOCK",
            r"\{[\t ]*\r?\n(?:[\S\s]*?\r?\n)?\}(?:\r?\n|(?!\S))",
            None,
        ),
        acc_descr("EM_ACC_DESCR"),
        acc_title().named("EM_ACC_TITLE"),
        title().named("EM_TITLE"),
        TokenType::regex("EM_WS", r"\s+", None).hidden(),
        yaml().named("EM_YAML"),
        directive().named("EM_DIRECTIVE"),
        single_line_comment().named("EM_SINGLE_LINE_COMMENT"),
        TokenType::regex("EM_ML_COMMENT", r"\/\*[\s\S]*?\*\/", None).hidden(),
        TokenType::regex("EM_SL_COMMENT", r"\/\/[^\n\r]*", None).hidden(),
    ]);
    Table::new(types)
});

/// Token kinds by name, as the parser reads them.
struct Kinds {
    keyword: i32,
    arrow: i32,
    open: i32,
    close: i32,
    tick: i32,
    dot: i32,
    id: i32,
    frame_id: i32,
    inline: i32,
    block: i32,
    acc_descr: i32,
    acc_title: i32,
    title: i32,
    entity: i32,
    data: i32,
    note: i32,
    gwt: i32,
    given: i32,
    when: i32,
    then: i32,
    time_frame: [i32; 2],
    reset_frame: [i32; 2],
    entity_type: Vec<i32>,
    data_type: Vec<i32>,
}

static T: LazyLock<Kinds> = LazyLock::new(|| {
    let table = &*EVENTMODELING_TOKENS;
    let index = |name: &str| table.index(name);
    let kinds = |names: &[&str]| names.iter().map(|name| index(name)).collect::<Vec<_>>();
    Kinds {
        keyword: 0,
        arrow: index("->>"),
        open: index("[["),
        close: index("]]"),
        tick: index("`"),
        dot: index("."),
        id: index("EM_ID"),
        frame_id: index("EM_FID"),
        inline: index("EM_DATA_INLINE"),
        block: index("EM_DATA_BLOCK"),
        acc_descr: index("EM_ACC_DESCR"),
        acc_title: index("EM_ACC_TITLE"),
        title: index("EM_TITLE"),
        entity: index("entity"),
        data: index("data"),
        note: index("note"),
        gwt: index("gwt"),
        given: index("given"),
        when: index("when"),
        then: index("then"),
        time_frame: [index("tf"), index("timeframe")],
        reset_frame: [index("rf"), index("resetframe")],
        entity_type: kinds(&[
            "rmo",
            "readmodel",
            "ui",
            "cmd",
            "command",
            "evt",
            "event",
            "pcr",
            "processor",
        ]),
        data_type: kinds(&[
            "json", "jsobj", "figma", "salt", "uri", "md", "html", "text",
        ]),
    }
});

/// Langium gives every keyword that is also an identifier the identifier as
/// a longer alternative.
fn word(text: &'static str) -> TokenType {
    TokenType::literal(text, text).longer(&["EM_ID"])
}

fn sign(text: &'static str) -> TokenType {
    TokenType::literal(text, text)
}

pub struct EmFrame {
    pub reset: bool,
    pub name: String,
    pub model_entity_type: String,
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
    /// The whole token, keyword included: the grammar's own terminals get no
    /// value conversion.
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub model_entities: Vec<String>,
    pub frames: Vec<EmFrame>,
    pub data_entities: Vec<EmDataEntity>,
    pub note_entities: Vec<EmNoteEntity>,
    pub gwt_entities: Vec<EmGwt>,
    pub source: String,
}

fn one<'s>(r: &mut Reader<'_, 's>, set: &[i32], expected: &str) -> Result<&'s str, Error> {
    if set.contains(&r.kind()) {
        Ok(r.take())
    } else {
        Err(r.fail(expected))
    }
}

fn qualified_name(r: &mut Reader) -> Result<String, Error> {
    let mut name = r.expect(T.id)?.to_string();
    while r.accept(T.dot) {
        name.push('.');
        name.push_str(r.expect(T.id)?);
    }
    Ok(name)
}

fn data_type(r: &mut Reader) -> Result<Option<String>, Error> {
    if !r.accept(T.tick) {
        return Ok(None);
    }
    let kind = one(r, &T.data_type, "a data type")?.to_string();
    r.expect(T.tick)?;
    Ok(Some(kind))
}

fn statements(r: &mut Reader) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    loop {
        one(r, &T.entity_type, "an entity type")?;
        out.push(r.expect(T.id)?.to_string());
        if !T.entity_type.contains(&r.kind()) {
            break;
        }
    }
    Ok(out)
}

pub fn parse_event_model(src: &str) -> Result<EventModelAst, Error> {
    let tokens = tokenize(src, &EVENTMODELING_TOKENS, Kind::Eventmodeling)?;
    let mut r = Reader::new(&tokens, &EVENTMODELING_TOKENS, Kind::Eventmodeling);
    let mut ast = EventModelAst {
        source: src.to_string(),
        ..EventModelAst::default()
    };

    r.expect(T.keyword)?;
    while r.kind() != -1 {
        let kind = r.kind();
        if kind == T.acc_descr {
            ast.acc_descr = Some(r.take().to_string());
        } else if kind == T.acc_title {
            ast.acc_title = Some(r.take().to_string());
        } else if kind == T.title {
            ast.title = Some(r.take().to_string());
        } else if kind == T.entity {
            r.i += 1;
            ast.model_entities.push(qualified_name(&mut r)?);
        } else if T.time_frame.contains(&kind) || T.reset_frame.contains(&kind) {
            let offset = r.tokens.starts[r.i];
            r.i += 1;
            let name = r.expect(T.frame_id)?.to_string();
            let model_entity_type = one(&mut r, &T.entity_type, "an entity type")?.to_string();
            let entity_identifier = qualified_name(&mut r)?;
            let mut frame = EmFrame {
                reset: !T.time_frame.contains(&kind),
                name,
                model_entity_type,
                entity_identifier,
                source_frames: Vec::new(),
                data_reference: None,
                data_type: None,
                data_inline_value: None,
                offset,
            };
            while r.accept(T.arrow) {
                frame.source_frames.push(r.expect(T.frame_id)?.to_string());
            }
            if r.accept(T.open) {
                frame.data_reference = Some(r.expect(T.id)?.to_string());
                r.expect(T.close)?;
            }
            if r.kind() == T.tick || r.kind() == T.inline {
                if let Some(kind) = data_type(&mut r)? {
                    frame.data_type = Some(kind);
                }
                frame.data_inline_value = Some(r.expect(T.inline)?.to_string());
            }
            ast.frames.push(frame);
        } else if kind == T.data {
            r.i += 1;
            let name = r.expect(T.id)?.to_string();
            let data_type = data_type(&mut r)?;
            let data_block_value = r.expect(T.block)?.to_string();
            ast.data_entities.push(EmDataEntity {
                name,
                data_type,
                data_block_value,
            });
        } else if kind == T.note {
            r.i += 1;
            let source_frame = r.expect(T.frame_id)?.to_string();
            let data_type = data_type(&mut r)?;
            let data_block_value = r.expect(T.block)?.to_string();
            ast.note_entities.push(EmNoteEntity {
                source_frame,
                data_type,
                data_block_value,
            });
        } else if kind == T.gwt {
            r.i += 1;
            let source_frame = r.expect(T.frame_id)?.to_string();
            r.expect(T.given)?;
            let given_statements = statements(&mut r)?;
            let when_statements = if r.accept(T.when) {
                statements(&mut r)?
            } else {
                Vec::new()
            };
            r.expect(T.then)?;
            let then_statements = statements(&mut r)?;
            ast.gwt_entities.push(EmGwt {
                source_frame,
                given_statements,
                when_statements,
                then_statements,
            });
        } else {
            return Err(r.fail("a frame, an entity, a data block, a note, or a specification"));
        }
    }
    Ok(ast)
}
