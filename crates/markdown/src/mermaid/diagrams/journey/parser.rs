//! Pele's `diagrams/journey/parser.ts`.

use super::{
    db::JourneyModel,
    lexer::{
        ACC_DESCR, ACC_DESCR_MULTILINE_VALUE, ACC_TITLE, END, EOF, JOURNEY, NEWLINE, SECTION,
        TASK_DATA, TASK_NAME, TITLE, TOKEN_NAMES, tokenize,
    },
};
use crate::mermaid::{Error, detect::Kind, js, util::chars::width_at};

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskName'";

/// `text` past its keyword, `keyword` bytes long, and the space after it.
fn after_keyword(text: &str, keyword: usize) -> &str {
    &text[keyword + width_at(text, keyword)..]
}

pub fn parse_journey(src: &str, db: &mut JourneyModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let (types, texts, starts) = (&tokens.types, &tokens.texts, &tokens.starts);

    let fail = |i: usize, expected: &str| -> Error {
        let got = if types[i] == END {
            "1"
        } else {
            TOKEN_NAMES[types[i] as usize]
        };
        Error::at(
            Kind::Journey,
            src,
            starts[i],
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    };

    if types[0] != JOURNEY {
        return Err(fail(0, "'journey'"));
    }

    let mut i = 1;
    loop {
        let text = &texts[i];
        match types[i] {
            NEWLINE => {}
            TITLE => db.set_diagram_title(after_keyword(text, 5)),
            ACC_TITLE => {
                i += 1;
                db.set_acc_title(js::trim(&texts[i]));
            }
            ACC_DESCR => {
                i += 1;
                db.set_acc_description(js::trim(&texts[i]));
            }
            ACC_DESCR_MULTILINE_VALUE => db.set_acc_description(js::trim(text)),
            SECTION => db.add_section(after_keyword(text, 7)),
            TASK_NAME => {
                i += 1;
                if types[i] != TASK_DATA {
                    return Err(fail(i, "'taskData'"));
                }
                db.add_task(text, &texts[i]);
            }
            EOF => return Ok(()),
            _ => return Err(fail(i, STATEMENT)),
        }
        i += 1;
    }
}
