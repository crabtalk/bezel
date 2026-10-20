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

/// Mermaid's parser runs a statement's action only after seeing that the
/// next token can follow it; `also` names what else could have.
fn line_end(s: &Stream<'_, '_, T>, also: &str) -> Result<(), Error> {
    if s.kind().starts_line() {
        Ok(())
    } else {
        Err(s.fail(&format!("{LINE}{also}")))
    }
}

pub fn parse_gantt(src: &str, db: &mut GanttDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::Gantt, src, &tokens);
    s.expect(T::Gantt, "")?;

    loop {
        let kind = s.kind();
        if !kind.starts_line() {
            return Err(s.fail(LINE));
        }
        let text = s.take();
        match kind {
            T::Eof => {
                if s.is(T::End) {
                    return Ok(());
                }
                line_end(&s, "")?;
            }
            T::Nl => {}
            T::AccTitle => {
                let value = s.expect(T::AccTitleValue, "")?;
                line_end(&s, "")?;
                db.titles.set_acc_title(js::trim(value));
            }
            T::AccDescr => {
                let value = s.expect(T::AccDescrValue, "")?;
                line_end(&s, "")?;
                db.titles.set_acc_descr(js::trim(value));
            }
            T::TaskTxt => {
                let data = s.expect(T::TaskData, "")?;
                line_end(&s, "")?;
                db.add_task(text, data)?;
            }
            T::Click => click(&mut s, db, text)?,
            _ => {
                line_end(&s, "")?;
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

/// `click ids` then a callback, a link, or both in either order.
fn click(s: &mut Stream<'_, '_, T>, db: &mut GanttDb, ids: &str) -> Result<(), Error> {
    let mut callback = false;
    let mut href = None;
    if s.eat(T::Callbackname) {
        callback = true;
        let args = s.eat(T::Callbackargs);
        if s.is(T::Href) {
            href = Some(s.take());
            line_end(s, "")?;
        } else {
            line_end(
                s,
                if args {
                    ", 'href'"
                } else {
                    ", 'callbackargs', 'href'"
                },
            )?;
        }
    } else if s.is(T::Href) {
        href = Some(s.take());
        if s.eat(T::Callbackname) {
            callback = true;
            if s.eat(T::Callbackargs) {
                line_end(s, "")?;
            } else {
                line_end(s, ", 'callbackargs'")?;
            }
        } else {
            line_end(s, ", 'callbackname'")?;
        }
    } else {
        return Err(s.fail("'callbackname', 'href'"));
    }
    if callback {
        db.set_click_event(ids);
    }
    if let Some(href) = href {
        db.set_link(ids, href);
    }
    Ok(())
}
