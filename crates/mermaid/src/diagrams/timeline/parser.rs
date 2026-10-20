//! Pele's `diagrams/timeline/parser.ts`.

use super::{db::TimelineModel, lexer::tokenize};
use crate::{
    Error,
    detect::Kind,
    diagrams::common::scan::T,
    geom::Direction,
    js,
    token::{Stream, after_keyword},
};

const STATEMENT: &str = "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'period', 'event'";

pub fn parse_timeline(src: &str, db: &mut TimelineModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::Timeline, src, &tokens);
    match s.kind() {
        T::Timeline => {}
        T::TimelineLr => db.direction = Direction::LeftToRight,
        T::TimelineTd => db.direction = Direction::TopToBottom,
        _ => return Err(s.fail_expecting([T::Timeline, T::TimelineLr, T::TimelineTd])),
    }
    s.bump();
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
            T::Period => db.add_period(text),
            T::Event => db.add_event(after_keyword(text, 1))?,
            T::Eof => return Ok(()),
            _ => return Err(s.fail(STATEMENT)),
        }
        s.bump();
    }
}
