use std::{cell::RefCell, rc::Rc};

use gpui::{Entity, SharedString, TestAppContext, VisualTestContext, px, size};
use ui::{
    input,
    multi_select::{self, Check, Choice, MultiSelect, MultiSelectEvent},
};

type Events = Rc<RefCell<Vec<MultiSelectEvent>>>;

fn choice(name: &str, check: Check) -> Choice {
    Choice {
        name: name.into(),
        check,
        count: None,
    }
}

fn normalize(text: &str) -> Option<SharedString> {
    let name = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    (!name.is_empty()).then(|| name.into())
}

fn picker(
    cx: &mut TestAppContext,
    choices: Vec<Choice>,
) -> (Entity<MultiSelect>, Events, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        input::init(cx);
        multi_select::init(cx);
    });
    let window = cx.add_window(|_, cx| MultiSelect::new(choices, cx).with_create(normalize));
    let picker = window.root(cx).unwrap();
    let events = Events::default();
    cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&picker, move |_, event: &MultiSelectEvent, _| {
            events.borrow_mut().push(event.clone())
        })
        .detach();
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(400.0)));
    visual.update(|window, cx| picker.update(cx, |picker, cx| picker.reset(window, cx)));
    visual.run_until_parked();
    (picker, events, visual)
}

#[gpui::test]
fn enter_on_a_new_name_creates_it_normalised(cx: &mut TestAppContext) {
    let (_, events, mut cx) = picker(cx, vec![choice("design", Check::Off)]);
    cx.simulate_input("Q3 Plan");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        *events.borrow(),
        [MultiSelectEvent::Created("q3-plan".into())]
    );
}

#[gpui::test]
fn a_name_already_offered_is_toggled_not_created(cx: &mut TestAppContext) {
    let (_, events, mut cx) = picker(cx, vec![choice("design", Check::Off)]);
    cx.simulate_input("Design");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        *events.borrow(),
        [MultiSelectEvent::Toggled {
            name: "design".into(),
            on: true
        }]
    );
}

#[gpui::test]
fn a_mixed_choice_turns_on(cx: &mut TestAppContext) {
    let (_, events, mut cx) = picker(cx, vec![choice("design", Check::Mixed)]);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        *events.borrow(),
        [MultiSelectEvent::Toggled {
            name: "design".into(),
            on: true
        }]
    );
}

#[gpui::test]
fn backspace_in_an_empty_query_turns_off_the_last_choice_on(cx: &mut TestAppContext) {
    let (_, events, mut cx) = picker(
        cx,
        vec![
            choice("design", Check::On),
            choice("q3", Check::On),
            choice("urgent", Check::Mixed),
        ],
    );
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace");
    assert!(
        events.borrow().is_empty(),
        "the first backspace edits the query"
    );
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        *events.borrow(),
        [MultiSelectEvent::Toggled {
            name: "q3".into(),
            on: false
        }]
    );
}

#[gpui::test]
fn escape_asks_to_be_dismissed(cx: &mut TestAppContext) {
    let (_, events, mut cx) = picker(cx, vec![choice("design", Check::Off)]);
    cx.simulate_keystrokes("escape");
    assert_eq!(*events.borrow(), [MultiSelectEvent::Dismissed]);
}
