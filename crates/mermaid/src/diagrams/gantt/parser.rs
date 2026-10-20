//! Pele's `diagrams/gantt/parser.ts`. The calls the grammar's actions make
//! are [`GanttDb`]'s methods, under Mermaid's names.

use super::{
    db::{GanttDb, Weekend},
    lexer::tokenize,
    tokens::T,
};
use crate::{
    Error,
    detect::Kind,
    js,
    token::{Stream, after_keyword},
};

const LINE: &str = "'EOF', 'SPACE', 'NL', 'weekday_monday', 'weekday_tuesday', 'weekday_wednesday', 'weekday_thursday', 'weekday_friday', 'weekday_saturday', 'weekday_sunday', 'weekend_friday', 'weekend_saturday', 'dateFormat', 'inclusiveEndDates', 'topAxis', 'axisFormat', 'tickInterval', 'excludes', 'includes', 'todayMarker', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskTxt', 'click'";

pub fn parse_gantt(src: &str, db: &mut GanttDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut p = Parser {
        s: Stream::new(Kind::Gantt, src, &tokens),
    };
    p.s.expect(T::Gantt, "")?;

    loop {
        let kind = p.s.kind();
        if !kind.starts_line() {
            return Err(p.s.fail(LINE));
        }
        let text = p.s.take();
        match kind {
            T::Eof => {
                if p.s.is(T::End) {
                    return Ok(());
                }
                p.line_end("")?;
            }
            T::Nl => {}
            T::AccTitle => {
                let value = p.s.expect(T::AccTitleValue, "")?;
                p.line_end("")?;
                db.titles.set_acc_title(js::trim(value));
            }
            T::AccDescr => {
                let value = p.s.expect(T::AccDescrValue, "")?;
                p.line_end("")?;
                db.titles.set_acc_descr(js::trim(value));
            }
            T::TaskTxt => {
                let data = p.s.expect(T::TaskData, "")?;
                p.line_end("")?;
                db.add_task(text, data)?;
            }
            T::Click => p.click(db, text)?,
            _ => {
                p.line_end("")?;
                match kind {
                    T::DateFormat => db.set_date_format(after_keyword(text, 10)),
                    T::InclusiveEndDates => db.enable_inclusive_end_dates(),
                    T::TopAxis => db.top_axis(),
                    T::AxisFormat => db.set_axis_format(after_keyword(text, 10)),
                    T::TickInterval => db.set_tick_interval(after_keyword(text, 12)),
                    T::Excludes => db.set_excludes(after_keyword(text, 8)),
                    T::Includes => db.set_includes(after_keyword(text, 8)),
                    T::TodayMarker => db.set_today_marker(after_keyword(text, 11)),
                    T::WeekendFriday => db.weekend = Weekend::Friday,
                    T::WeekendSaturday => db.weekend = Weekend::Saturday,
                    T::Title => db.titles.set_title(after_keyword(text, 5)),
                    T::AccDescrMultilineValue => db.titles.set_acc_descr(js::trim(text)),
                    T::Section => db.add_section(after_keyword(text, 7)),
                    _ => {
                        if let Some(day) = kind.weekday() {
                            db.set_weekday(day);
                        }
                    }
                }
            }
        }
    }
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl<'t> Parser<'t, '_> {
    /// Mermaid's parser runs a statement's action only after seeing that the
    /// next token can follow it; `also` names what else could have.
    fn line_end(&self, also: &str) -> Result<(), Error> {
        if self.s.kind().starts_line() {
            Ok(())
        } else {
            Err(self.s.fail(&format!("{LINE}{also}")))
        }
    }

    /// `click ids` then a callback, a link, or both in either order.
    fn click(&mut self, db: &mut GanttDb, ids: &str) -> Result<(), Error> {
        let mut callback = false;
        let mut href = None;
        if self.s.eat(T::Callbackname) {
            callback = true;
            let args = self.s.eat(T::Callbackargs);
            if self.s.is(T::Href) {
                href = Some(self.s.take());
                self.line_end("")?;
            } else {
                self.line_end(if args {
                    ", 'href'"
                } else {
                    ", 'callbackargs', 'href'"
                })?;
            }
        } else if self.s.is(T::Href) {
            href = Some(self.s.take());
            if self.s.eat(T::Callbackname) {
                callback = true;
                if self.s.eat(T::Callbackargs) {
                    self.line_end("")?;
                } else {
                    self.line_end(", 'callbackargs'")?;
                }
            } else {
                self.line_end(", 'callbackname'")?;
            }
        } else {
            return Err(self.s.fail("'callbackname', 'href'"));
        }
        if callback {
            db.set_click_event(ids);
        }
        if let Some(href) = href {
            db.set_link(ids, href);
        }
        Ok(())
    }
}
