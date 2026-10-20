//! Pele's `diagrams/journey/parser.ts`.

use super::{db::JourneyModel, lexer::tokenize};
use crate::{
    Error,
    detect::Kind,
    diagrams::common::scan::T,
    js,
    token::{Stream, after_keyword},
};

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskName'";

pub fn parse_journey(src: &str, db: &mut JourneyModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::Journey, src, &tokens);
    s.expect(T::Journey, "")?;
    loop {
        let text = s.text();
        match s.kind() {
            T::Newline => {}
            T::Title => db.titles.set_title(after_keyword(text, 5)),
            T::AccTitle => {
                s.bump();
                db.titles.set_acc_title(js::trim(s.text()));
            }
            T::AccDescr => {
                s.bump();
                db.titles.set_acc_descr(js::trim(s.text()));
            }
            T::AccDescrMultilineValue => db.titles.set_acc_descr(js::trim(text)),
            T::Section => db.add_section(after_keyword(text, 7)),
            T::TaskName => {
                s.bump();
                db.add_task(text, s.expect(T::TaskData, "")?);
                continue;
            }
            T::Eof => return Ok(()),
            _ => return Err(s.fail(STATEMENT)),
        }
        s.bump();
    }
}
