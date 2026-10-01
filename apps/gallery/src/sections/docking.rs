use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use motion::Painter;
use theme::Theme;
use ui::{
    docking::{Dock, Drop, Zone},
    tabs,
};

pub(crate) struct Demo {
    dock: Dock<usize, &'static str>,
    panes: Vec<Pane>,
    layout: Layout,
    next: usize,
}
struct Pane {
    id: usize,
    tabs: tabs::Strip<&'static str>,
    reorder: tabs::Reorder<&'static str>,
}
#[derive(Clone)]
enum Layout {
    Pane(usize),
    Split {
        vertical: bool,
        first: Box<Self>,
        second: Box<Self>,
    },
}
impl Layout {
    fn split(&mut self, target: usize, new: usize, zone: Zone) {
        match self {
            Self::Pane(id) if *id == target => {
                let (first, second) = if matches!(zone, Zone::Left | Zone::Top) {
                    (new, target)
                } else {
                    (target, new)
                };
                *self = Self::Split {
                    vertical: matches!(zone, Zone::Top | Zone::Bottom),
                    first: Box::new(Self::Pane(first)),
                    second: Box::new(Self::Pane(second)),
                };
            }
            Self::Split { first, second, .. } => {
                first.split(target, new, zone);
                second.split(target, new, zone);
            }
            _ => {}
        }
    }
    fn remove(self, removed: usize) -> Option<Self> {
        match self {
            Self::Pane(id) => (id != removed).then_some(Self::Pane(id)),
            Self::Split {
                vertical,
                first,
                second,
            } => match (first.remove(removed), second.remove(removed)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    vertical,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (one, two) => one.or(two),
            },
        }
    }
}
impl Demo {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let painter = Painter::of(cx);
        Self {
            dock: Dock::new(painter, |id, _, cx| {
                tabs::tab(
                    Theme::of(cx),
                    *id,
                    tabs::Label::new(*id),
                    tabs::State::Front,
                )
                .into_any_element()
            }),
            panes: [vec!["Overview", "Notes", "Roadmap"], vec!["Preview"]]
                .into_iter()
                .enumerate()
                .map(|(id, items)| Pane {
                    id,
                    tabs: items.into_iter().collect(),
                    reorder: tabs::Reorder::new(painter),
                })
                .collect(),
            layout: Layout::Split {
                vertical: false,
                first: Box::new(Layout::Pane(0)),
                second: Box::new(Layout::Pane(1)),
            },
            next: 2,
        }
    }
    fn drop(&mut self, event: &Drop<usize, &'static str>, cx: &mut Context<Self>) -> Option<usize> {
        let source = self
            .panes
            .iter()
            .position(|pane| pane.tabs.contains(&event.item))?;
        let target = self.panes.iter().position(|pane| pane.id == event.pane)?;
        if source == target && (event.zone == Zone::Join || self.panes[source].tabs.len() == 1) {
            return None;
        }
        self.panes[source].tabs.close(&event.item);
        let destination = if event.zone == Zone::Join {
            self.panes[target].tabs.open(event.item);
            event.pane
        } else {
            let id = self.next;
            self.next += 1;
            self.panes.push(Pane {
                id,
                tabs: [event.item].into_iter().collect(),
                reorder: tabs::Reorder::new(Painter::of(cx)),
            });
            self.layout.split(event.pane, id, event.zone);
            id
        };
        if self.panes[source].tabs.is_empty() {
            let removed = self.panes.remove(source).id;
            self.layout = self.layout.clone().remove(removed).unwrap();
        }
        cx.notify();
        Some(destination)
    }
    fn render_layout(&self, layout: &Layout, cx: &Context<Self>) -> AnyElement {
        match layout {
            Layout::Split {
                vertical,
                first,
                second,
            } => div()
                .size_full()
                .min_w_0()
                .min_h_0()
                .flex()
                .when(*vertical, |el| el.flex_col())
                .gap(px(6.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(self.render_layout(first, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(self.render_layout(second, cx)),
                )
                .into_any_element(),
            Layout::Pane(id) => {
                let id = *id;
                let pane = self.panes.iter().find(|pane| pane.id == id).unwrap();
                let theme = Theme::of(cx);
                let bar = pane
                    .reorder
                    .bar(
                        ("pane-tabs", id),
                        &pane.tabs,
                        pane.tabs.tabs().iter().map(|&item| {
                            (
                                item,
                                tabs::tab(
                                    theme,
                                    item,
                                    tabs::Label::new(item),
                                    if pane.tabs.active() == Some(&item) {
                                        tabs::State::Front
                                    } else {
                                        tabs::State::Resting
                                    },
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        if let Some(pane) =
                                            view.panes.iter_mut().find(|pane| pane.id == id)
                                        {
                                            pane.tabs.activate(&item);
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                        }),
                    )
                    .on_reorder(cx.listener(move |view, movement: &tabs::Move, _, cx| {
                        if let Some(pane) = view.panes.iter_mut().find(|pane| pane.id == id) {
                            pane.tabs.reorder(movement.from, movement.to);
                        }
                        cx.notify();
                    }));
                self.dock
                    .pane(
                        id,
                        px(26.),
                        div()
                            .size_full()
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(6.))
                            .child(bar)
                            .child(
                                div()
                                    .p(px(20.))
                                    .text_color(theme.text_muted)
                                    .child(*pane.tabs.active().unwrap())
                                    .child(
                                        div()
                                            .mt(px(12.))
                                            .child("Drag a tab to rearrange this workspace."),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
        }
    }
}
impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        self.dock
            .surface("dock-demo", self.render_layout(&self.layout, cx))
            .on_drop(move |event, _, cx| {
                entity
                    .update(cx, |view, cx| view.drop(event, cx))
                    .ok()
                    .flatten()
            })
    }
}
