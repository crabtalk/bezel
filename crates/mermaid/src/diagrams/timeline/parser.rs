//! Pele's `diagrams/timeline/parser.ts`.

use super::{
    db::TimelineModel,
    lexer::{
        ACC_DESCR, ACC_DESCR_MULTILINE_VALUE, ACC_TITLE, END, EOF, EVENT, NEWLINE, PERIOD, SECTION,
        TIMELINE, TIMELINE_LR, TIMELINE_TD, TITLE, TOKEN_NAMES, tokenize,
    },
};
use crate::{Error, detect::Kind, js, util::chars::width_at};

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'period', 'event'";

/// `text` past its keyword, `keyword` bytes long, and the space after it.
fn after_keyword(text: &str, keyword: usize) -> &str {
    text.get(keyword + width_at(text, keyword)..).unwrap_or("")
}

pub fn parse_timeline(src: &str, db: &mut TimelineModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let (types, texts, starts) = (&tokens.types, &tokens.texts, &tokens.starts);

    let fail = |i: usize, expected: &str| -> Error {
        let got = if types[i] == END {
            "1"
        } else {
            TOKEN_NAMES[types[i] as usize]
        };
        Error::at(
            Kind::Timeline,
            src,
            starts[i],
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    };

    let head = types[0];
    if head != TIMELINE && head != TIMELINE_LR && head != TIMELINE_TD {
        return Err(fail(0, "'timeline', 'timeline_lr', 'timeline_td'"));
    }
    if head != TIMELINE {
        db.set_direction(if head == TIMELINE_LR { "LR" } else { "TD" });
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
            PERIOD => db.add_task(text, 0.0, Some("")),
            EVENT => db.add_event(after_keyword(text, 1))?,
            EOF => return Ok(()),
            _ => return Err(fail(i, STATEMENT)),
        }
        i += 1;
    }
}
