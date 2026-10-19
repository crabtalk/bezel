//! Pele's `diagrams/gantt/parser.ts`. The calls the grammar's actions make
//! are [`GanttDb`]'s methods, under Mermaid's names.

use super::{db::GanttDb, lexer::tokenize, tokens::*};
use crate::{Error, detect::Kind, js, util::chars::width_at};

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];
const LINE: &str = "'EOF', 'SPACE', 'NL', 'weekday_monday', 'weekday_tuesday', 'weekday_wednesday', 'weekday_thursday', 'weekday_friday', 'weekday_saturday', 'weekday_sunday', 'weekend_friday', 'weekend_saturday', 'dateFormat', 'inclusiveEndDates', 'topAxis', 'axisFormat', 'tickInterval', 'excludes', 'includes', 'todayMarker', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskTxt', 'click'";

/// Tokens that can start a line, which are also the only ones that can
/// follow a complete statement.
fn starts_line(kind: i32) -> bool {
    (WEEKDAY_MONDAY..=WEEKEND_SATURDAY).contains(&kind)
        || matches!(
            kind,
            EOF | NL
                | DATE_FORMAT
                | INCLUSIVE_END_DATES
                | TOP_AXIS
                | AXIS_FORMAT
                | TICK_INTERVAL
                | EXCLUDES
                | INCLUDES
                | TODAY_MARKER
                | TITLE
                | ACC_TITLE
                | ACC_DESCR
                | ACC_DESCR_MULTILINE_VALUE
                | SECTION
                | TASK_TXT
                | CLICK
        )
}

/// `text` past its keyword, `keyword` bytes long, and the space after it.
fn after_keyword(text: &str, keyword: usize) -> &str {
    text.get(keyword + width_at(text, keyword)..).unwrap_or("")
}

pub fn parse_gantt(src: &str, db: &mut GanttDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let (types, texts, starts) = (&tokens.types, &tokens.texts, &tokens.starts);
    let mut i = 0;

    let fail = |i: usize, expected: &str| -> Error {
        let t = types[i];
        let got = if t == END {
            "1"
        } else {
            TOKEN_NAMES[t as usize]
        };
        Error::at(
            Kind::Gantt,
            src,
            starts[i],
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    };
    // Mermaid's parser runs a statement's action only after seeing that the
    // next token can follow it.
    let end = |i: usize, also: &str| -> Result<(), Error> {
        if starts_line(types[i]) {
            Ok(())
        } else {
            Err(fail(i, &format!("{LINE}{also}")))
        }
    };

    if types[i] != GANTT {
        return Err(fail(i, "'gantt'"));
    }
    i += 1;

    loop {
        let t = types[i];
        let text = texts[i].as_str();
        if !starts_line(t) {
            return Err(fail(i, LINE));
        }
        i += 1;
        match t {
            EOF => {
                if types[i] == END {
                    return Ok(());
                }
                end(i, "")?;
            }
            NL => {}
            DATE_FORMAT => {
                end(i, "")?;
                db.set_date_format(after_keyword(text, 10));
            }
            INCLUSIVE_END_DATES => {
                end(i, "")?;
                db.enable_inclusive_end_dates();
            }
            TOP_AXIS => {
                end(i, "")?;
                db.top_axis();
            }
            AXIS_FORMAT => {
                end(i, "")?;
                db.set_axis_format(after_keyword(text, 10));
            }
            TICK_INTERVAL => {
                end(i, "")?;
                db.set_tick_interval(after_keyword(text, 12));
            }
            EXCLUDES => {
                end(i, "")?;
                db.set_excludes(after_keyword(text, 8));
            }
            INCLUDES => {
                end(i, "")?;
                db.set_includes(after_keyword(text, 8));
            }
            TODAY_MARKER => {
                end(i, "")?;
                db.set_today_marker(after_keyword(text, 11));
            }
            WEEKEND_FRIDAY => {
                end(i, "")?;
                db.set_weekend("friday");
            }
            WEEKEND_SATURDAY => {
                end(i, "")?;
                db.set_weekend("saturday");
            }
            TITLE => {
                end(i, "")?;
                db.set_diagram_title(after_keyword(text, 5));
            }
            ACC_TITLE => {
                if types[i] != ACC_TITLE_VALUE {
                    return Err(fail(i, "'acc_title_value'"));
                }
                let value = &texts[i];
                i += 1;
                end(i, "")?;
                db.set_acc_title(js::trim(value));
            }
            ACC_DESCR => {
                if types[i] != ACC_DESCR_VALUE {
                    return Err(fail(i, "'acc_descr_value'"));
                }
                let value = &texts[i];
                i += 1;
                end(i, "")?;
                db.set_acc_description(js::trim(value));
            }
            ACC_DESCR_MULTILINE_VALUE => {
                end(i, "")?;
                db.set_acc_description(js::trim(text));
            }
            SECTION => {
                end(i, "")?;
                db.add_section(after_keyword(text, 7));
            }
            TASK_TXT => {
                if types[i] != TASK_DATA {
                    return Err(fail(i, "'taskData'"));
                }
                let data = &texts[i];
                i += 1;
                end(i, "")?;
                db.add_task(text, data)?;
            }
            CLICK => {
                let mut name: Option<&str> = None;
                let mut args: Option<&str> = None;
                let mut href: Option<&str> = None;
                if types[i] == CALLBACKNAME {
                    name = Some(&texts[i]);
                    i += 1;
                    if types[i] == CALLBACKARGS {
                        args = Some(&texts[i]);
                        i += 1;
                    }
                    if types[i] == HREF {
                        href = Some(&texts[i]);
                        i += 1;
                        end(i, "")?;
                    } else {
                        end(
                            i,
                            if args.is_none() {
                                ", 'callbackargs', 'href'"
                            } else {
                                ", 'href'"
                            },
                        )?;
                    }
                } else if types[i] == HREF {
                    href = Some(&texts[i]);
                    i += 1;
                    if types[i] == CALLBACKNAME {
                        name = Some(&texts[i]);
                        i += 1;
                        if types[i] == CALLBACKARGS {
                            args = Some(&texts[i]);
                            i += 1;
                            end(i, "")?;
                        } else {
                            end(i, ", 'callbackargs'")?;
                        }
                    } else {
                        end(i, ", 'callbackname'")?;
                    }
                } else {
                    return Err(fail(i, "'callbackname', 'href'"));
                }
                if let Some(name) = name {
                    db.set_click_event(text, name, args);
                }
                if let Some(href) = href {
                    db.set_link(text, href);
                }
            }
            _ => {
                end(i, "")?;
                db.set_weekday(WEEKDAYS[(t - WEEKDAY_MONDAY) as usize]);
            }
        }
    }
}
