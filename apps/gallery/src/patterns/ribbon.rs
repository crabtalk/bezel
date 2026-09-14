//! The formatting ribbon — the bar that is always on.
//!
//! Nothing here is library code. The whole bar is one call: `editor.formatting()`
//! answers every question it paints, and each control is the entry point the
//! chord already takes, so a button and a keystroke cannot disagree.
//!
//! Three things it is built to show.
//!
//! **One read, not four.** Which marks are lit, what the block is called,
//! whether `cmd-E` would fence and whether any of it applies are taken together
//! at the top of the render. A bar that answered half of them from this frame
//! and half from the last would light the wrong button for a frame.
//!
//! **The block menu is the library's vocabulary.** `editor::turns()` is the
//! same list the slash menu and the gutter handle offer, so this dropdown
//! cannot drift from the two menus bezel opens itself.
//!
//! **Disabled is reported, not guessed.** In the source there is no markup to
//! toggle — the editor refuses the call — so the bar reads `formatting.mode`
//! and greys itself out rather than lighting buttons that would do nothing.
//!
//! The other answer to the same question is the bubble toolbar on the Editor
//! page, which appears at the selection instead. An app picks one: shipping
//! both is two places to keep in step.

use editor::{Editor, Formatting, Mode};
use gpui::{
    Context, Entity, Focusable, Render, ScrollHandle, SharedString, Window, div, prelude::*, px,
};
use markdown::Mark;
use motion::{Fade, Painter};
use theme::{TextStyle, Theme, Typeset};
use ui::{
    popover::{self, Popup},
    widgets::{ButtonStyle, Buttons, Controls},
};

/// Opens on prose with something to light: a heading, marks already in it, and
/// two lines to drag a selection across for `cmd-E`.
const SOURCE: &str = r#"# Release notes

The ribbon reads **one** snapshot per frame. Put the caret in _any_ of this and every control above lights itself from it — the block it sits in, the marks it carries, and what `cmd-E` would do with the selection.

- Select across two lines to see the code button become a fence
- Switch to Markdown and the bar greys out: there is nothing there to toggle
"#;

/// The marks the bar offers, and the glyph each carries.
const MARKS: [(&[u8], &str, Mark); 4] = [
    (icons::glyph::Bold, "bold", Mark::Bold),
    (icons::glyph::Italic, "italic", Mark::Italic),
    (icons::glyph::Strikethrough, "strike", Mark::Strike),
    (icons::glyph::Code, "code", Mark::Code),
];

/// How wide the block dropdown and its menu sit.
const TURN_WIDTH: f32 = 150.0;

pub struct RibbonDemo {
    editor: Entity<Editor>,
    /// The "turn into" dropdown.
    turn: Popup<()>,
    scroll: ScrollHandle,
    /// Focus lands in the document the first time this page paints: a bar with
    /// no caret under it has nothing to report.
    focused: bool,
}

impl RibbonDemo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let scroll = ScrollHandle::new();
        let editor = cx.new({
            let scroll = scroll.clone();
            |cx| Editor::new(SOURCE, cx).with_scroll(scroll)
        });
        // The bar is painted from the editor's state, so it has to hear about
        // every keystroke — the same `observe` any host reading `source()` owes.
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        Self {
            editor,
            turn: Popup::default(),
            scroll,
            focused: false,
        }
    }

    /// The block dropdown: what the caret's block is called, and the menu of
    /// everything it could be instead.
    fn turns(&self, formatting: &Formatting, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let view = Painter::of(cx);
        let open = self.turn.is_open() || self.turn.is_closing();
        let label = formatting
            .block
            .clone()
            .unwrap_or_else(|| SharedString::from("—"));
        let block = self.editor.read(cx).selection().head.block;

        // A bar with nothing to act on still says what it is looking at.
        if formatting.mode == Mode::Source {
            return div()
                .w(px(TURN_WIDTH))
                .child(theme.select_trigger("Markdown").opacity(0.4));
        }

        div().w(px(TURN_WIDTH)).relative().child(
            popover::menu_trigger(
                div().id("ribbon-turn"),
                |this: &mut Self| &mut this.turn,
                |_| (),
                cx,
            )
            .child(theme.select_trigger(label))
            .when(open, |trigger| {
                trigger.child(popover::anchored_menu_below(
                    "ribbon-turn-menu",
                    popover::dismiss_on_out(
                        popover::popover_card(theme).w(px(TURN_WIDTH)),
                        |this: &mut Self| &mut this.turn,
                        cx,
                    )
                    .child(
                        div()
                            .id("ribbon-turn-rows")
                            .max_h(px(320.0))
                            .overflow_y_scroll()
                            .children(editor::turns().into_iter().map(|(row, kind)| {
                                popover::menu_row(
                                    theme,
                                    Some(&row) == formatting.block.as_ref(),
                                    Some(Fade::new(view, format!("ribbon-turn-{row}"))),
                                )
                                .id(SharedString::from(format!("ribbon-turn-row-{row}")))
                                .child(row)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        popover::close_popup(this, cx, |this| &mut this.turn);
                                        let kind = kind.clone();
                                        this.editor.update(cx, |editor, cx| {
                                            editor.set_block(block, kind, cx);
                                        });
                                    },
                                ))
                            })),
                    )
                    .into_any_element(),
                    self.turn.closing_since(),
                ))
            }),
        )
    }

    /// The mark cluster. Lit from `formatting.marks`, which at a collapsed
    /// caret is what the next character typed would carry — so `cmd-B` before
    /// typing leaves the button lit, the way it has to.
    fn marks(&self, formatting: &Formatting, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let view = Painter::of(cx);
        let live = formatting.mode == Mode::Blocks;
        theme
            .control_group()
            .children(MARKS.map(|(glyph, key, mark)| {
                let lit = formatting.marks.contains(&mark);
                let button = theme
                    .icon_button(
                        glyph,
                        ButtonStyle::Ghost,
                        live.then(|| Fade::new(view, format!("ribbon-{key}"))),
                    )
                    .id(SharedString::from(format!("ribbon-mark-{key}")))
                    .when(lit, |el| el.bg(theme.element_active).text_color(theme.text))
                    .when(!live, |el| el.opacity(0.4));
                match live {
                    true => button.on_click(cx.listener(move |this, _, _, cx| {
                        let mark = mark.clone();
                        this.editor
                            .update(cx, |editor, cx| editor.toggle_mark(mark, cx));
                    })),
                    false => button,
                }
            }))
    }
}

impl Render for RibbonDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        if !self.focused {
            self.focused = true;
            let handle = self.editor.read(cx).focus_handle(cx);
            handle.focus(window, cx);
        }
        let formatting = self.editor.read(cx).formatting();

        // What `cmd-E` and the code button mean right now. The same key spells
        // an inline span and a fence, and only the editor can say which — a bar
        // counting newlines for itself would be wrong about a table.
        let fence = div()
            .font_family(theme.font_mono.clone())
            .text_style(TextStyle::Caption2)
            .text_color(theme.text_faint)
            .child(match formatting.fenceable {
                true => "⌘E fences",
                false => "⌘E is inline code",
            });

        let modes = [("Blocks", Mode::Blocks), ("Markdown", Mode::Source)];
        let toggle = theme.toggle_group().children(modes.map(|(label, to)| {
            theme
                .toggle_group_item(label, formatting.mode == to)
                .id(gpui::ElementId::Name(format!("ribbon-mode-{label}").into()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.editor.update(cx, |editor, cx| editor.set_mode(to, cx));
                }))
        }));

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Docked, not floating: this bar reflows the document under it,
                // and `ui::control_bar` is for the kind that does not.
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .pb(px(12.0))
                    .border_b_1()
                    .border_color(theme.hairline(0.10))
                    .child(self.turns(&formatting, &theme, cx))
                    .child(self.marks(&formatting, &theme, cx))
                    .child(fence)
                    .child(div().flex_1())
                    .child(toggle),
            )
            .child(
                div()
                    .id("ribbon-document")
                    .flex_1()
                    .min_h_0()
                    .pt(px(16.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(self.editor.clone()),
            )
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // A press anywhere on the page belongs to the editor — a
                    // bar reporting on a caret nobody can see is no demo.
                    let handle = this.editor.read(cx).focus_handle(cx);
                    handle.focus(window, cx);
                }),
            )
    }
}
