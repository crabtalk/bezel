use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let layout = Layout::of(cx);
        let focused = self.focus_handle.is_focused(window);
        // The only place the blink starts: `caret_moved` drops the task, so the
        // next render brings it back in phase, lit beat first.
        if focused && ui::input::caret_blink(cx) {
            if self.blink.is_none() {
                self.start_blink(cx);
            }
        } else {
            self.blink = None;
            self.caret_on = true;
        }
        let selection = focused.then_some(self.selection);
        // gpui ends an outside file drag — left the window or released
        // elsewhere — without a drop here, so the indicator goes with it.
        if !cx.has_active_drag() {
            self.dropping = None;
        }

        // Typed text and IME reach an entity only through an input handler
        // registered during *paint*, against the bounds it should be anchored
        // to. There is no custom element here to do that from, so a zero-cost
        // canvas over the document supplies the paint phase. Without this the
        // key bindings still fire and nothing types.
        let handle = self.focus_handle.clone();
        let entity = cx.entity();
        let in_drag = self.in_drag();
        let input = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                // `on_mouse_move` hears the pointer only over this box, and a
                // drag goes on past it. Registering it is paint's alone.
                if in_drag {
                    let entity = entity.clone();
                    window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Bubble && event.dragging() {
                            entity.update(cx, |this, cx| this.drag_to(event.position, cx));
                        }
                    });
                }
                // The gutter handle is placed from positions recorded in window
                // coordinates, so the box they have to be measured against is
                // taken here — the one place that knows it.
                entity.update(cx, |this, _| {
                    this.origin = bounds.origin;
                    this.width = bounds.size.width;
                });
                window.handle_input(
                    &handle,
                    ElementInputHandler::new(bounds, entity.clone()),
                    cx,
                );
            },
        )
        .absolute()
        .size_full();

        // A tab stop, so the editor is reachable the same way every other
        // control in the library is.
        let handle = self.focus_handle.clone().tab_stop(true);

        div()
            // Stateful only so the pointer leaving can be heard: `on_hover` is
            // what tells the gutter handle to stop pointing at a block the
            // pointer left behind.
            .id("bezel-editor")
            // The mark is what keeps `tab`: without it traversal answers the
            // key first and the caret never sees it.
            .key_context(key_context())
            .track_focus(&handle)
            // Tracking focus does not take it. Without this, clicking into the
            // document blurs the editor instead of putting a caret in it, and
            // the caret vanishes on the first click.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // The handle's own listener runs first and claims the
                    // press; without the flag this would close the menu it
                    // just opened. `ui::popover::Popup` solves it the same way.
                    if std::mem::take(&mut this.press_claimed) {
                        this.focus_handle.clone().focus(window, cx);
                    } else {
                        this.pressed(
                            event.position,
                            event.click_count,
                            event.modifiers,
                            window,
                            cx,
                        );
                    }
                    // What `press` reads to skip a press this box already
                    // took. It also stops gpui's own focus transfer, which runs
                    // after this listener, so both arms focus by hand.
                    window.prevent_default();
                }),
            )
            // The drag has to be tracked from the container rather than from a
            // payload: a text selection has nothing to carry, and gpui's drag
            // payload is for things being dropped somewhere.
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                // Ahead of every drag branch below, because the pointer's shape
                // is about where it *is* rather than about what it is doing.
                let over_text = this.layouts.over_text(event.position);
                if over_text != this.over_text {
                    this.over_text = over_text;
                    cx.notify();
                }
                // A drag in flight is followed by the window-wide listener
                // `render` registers; otherwise the pointer only decides which
                // block wears the handle.
                if this.in_drag() {
                    return;
                }
                let hovered = this.layouts.block_at(event.position);
                if hovered != this.hovered {
                    this.hovered = hovered;
                    cx.notify();
                }
            }))
            // Both, because a release can land anywhere on screen and only the
            // first fires over the editor. A resize left running would leave
            // its stand-in picture painted over the document for good.
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, window, cx| {
                    this.dragging = false;
                    this.drop_resize(window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.dragging = false;
                    if this.drop_resize(window, cx) {
                        return;
                    }
                    let Some((from, to)) = this.lifted.take() else {
                        return;
                    };
                    if from == to {
                        // A press that never moved is a click, and a click on
                        // the handle is what opens the menu — unless that same
                        // press is what dismissed it, which the note taken on
                        // the way down is the only way to tell.
                        if !this.block_menu.take_press_was_open() {
                            this.block_menu.open((from, event.position));
                        }
                        return cx.notify();
                    }
                    this.edit(EditKind::Structure, cx, |this| {
                        // `move_block` steps one sibling at a time, so a drop
                        // several blocks away is that many steps. Bounded by
                        // the block count, which no drag can exceed.
                        let delta = if to > from { 1 } else { -1 };
                        let mut at = from;
                        // Each step is its own move, so each is its own delta —
                        // folding them into one would have to compose the
                        // hops, and they are already in order.
                        let mut deltas = Vec::new();
                        for _ in 0..this.doc.blocks.len() {
                            let span = this.doc.subtree(at);
                            let Some(next) = this.doc.move_block(at, delta) else {
                                break;
                            };
                            deltas.push(Delta::Moved {
                                at: span,
                                to: Some(next),
                            });
                            at = next;
                            if (delta > 0 && at >= to) || (delta < 0 && at <= to) {
                                break;
                            }
                        }
                        this.selection =
                            Selection::at(Cursor::new(at, Part::Body, 0).clamp(&this.doc));
                        deltas
                    });
                }),
            )
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(
                cx.listener(|this, _: &KillLine, _, cx| this.delete_to(true, Cursor::end, cx)),
            )
            .on_action(cx.listener(|this, _: &DeleteWordLeft, _, cx| {
                this.delete_to(false, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteWordRight, _, cx| {
                this.delete_to(true, Cursor::word_right, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteToHome, _, cx| {
                this.delete_to(false, |at, _| at.home(), cx)
            }))
            .on_action(cx.listener(Self::split_block))
            .on_action(cx.listener(Self::soft_break))
            .on_action(cx.listener(Self::insert_paragraph))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::increase_text_size))
            .on_action(cx.listener(Self::decrease_text_size))
            .on_action(cx.listener(Self::reset_text_size))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::confirm_url))
            .on_action(cx.listener(Self::cancel_url))
            // A file crossing the document lights the same indicator a lifted
            // block does, so a drop from outside lands where it looks like it
            // will.
            .on_drag_move(cx.listener(
                |this, event: &gpui::DragMoveEvent<gpui::ExternalPaths>, _, cx| {
                    let at = event.event.position;
                    let over = event
                        .bounds
                        .contains(&at)
                        .then(|| this.layouts.block_at(at))
                        .flatten();
                    if over != this.dropping {
                        this.dropping = over;
                        cx.notify();
                    }
                },
            ))
            .on_drop(
                cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                    this.focus_handle.clone().focus(window, cx);
                    this.drop_paths(paths, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleBold, _, cx| this.toggle_mark(Mark::Bold, cx)))
            .on_action(
                cx.listener(|this, _: &ToggleItalic, _, cx| this.toggle_mark(Mark::Italic, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleStrike, _, cx| this.toggle_mark(Mark::Strike, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleCode, _, cx| this.toggle_mark(Mark::Code, cx)))
            .on_action(cx.listener(|this, _: &ToggleHighlight, _, cx| {
                if this.marks.delimiter(HIGHLIGHT_MARK).is_some() {
                    this.toggle_mark(Mark::Custom(HIGHLIGHT_MARK.into()), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveBlockUp, _, cx| {
                this.move_block(this.cursor().block, -1, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveBlockDown, _, cx| {
                this.move_block(this.cursor().block, 1, cx)
            }))
            .on_action(cx.listener(|this, _: &DuplicateBlock, _, cx| {
                this.duplicate_block(this.cursor().block, cx)
            }))
            .on_action(cx.listener(|this, _: &RemoveBlock, _, cx| {
                this.remove_block(this.cursor().block, cx)
            }))
            // Motion is one method with a `Cursor` function and an "extend"
            // flag, so a shift variant cannot drift from the key it shadows.
            .on_action(cx.listener(|this, _: &Left, _, cx| this.moved(false, Cursor::left, cx)))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.moved(false, Cursor::right, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(false, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(true, false, cx)))
            .on_action(cx.listener(|this, _: &Home, _, cx| this.moved(false, line_home, cx)))
            .on_action(cx.listener(|this, _: &End, _, cx| this.moved(false, line_end, cx)))
            .on_action(cx.listener(|this, _: &DocumentStart, _, cx| {
                this.moved(false, |_, doc| Selection::all(doc).anchor, cx)
            }))
            .on_action(cx.listener(|this, _: &DocumentEnd, _, cx| {
                this.moved(false, |_, doc| Selection::all(doc).head, cx)
            }))
            .on_action(
                cx.listener(|this, _: &WordLeft, _, cx| this.moved(false, Cursor::word_left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &WordRight, _, cx| this.moved(false, Cursor::word_right, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectLeft, _, cx| this.moved(true, Cursor::left, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SelectRight, _, cx| this.moved(true, Cursor::right, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(true, true, cx)))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| this.moved(true, line_home, cx)))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.moved(true, line_end, cx)))
            .on_action(cx.listener(|this, _: &SelectDocumentStart, _, cx| {
                this.moved(true, |_, doc| Selection::all(doc).anchor, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentEnd, _, cx| {
                this.moved(true, |_, doc| Selection::all(doc).head, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| {
                this.moved(true, Cursor::word_left, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| {
                this.moved(true, Cursor::word_right, cx)
            }))
            .w_full()
            // Text under the pointer, so the pointer says so — and only there,
            // or while a drag is still sweeping one out. The editor's box
            // reaches over its gutter, the margin beside a short line, a rule,
            // an image and a card, none of which a caret can be put into.
            // Where it has nothing to say it stays quiet rather than
            // overriding the page with an arrow of its own.
            .when(self.over_text || self.dragging, |el| {
                el.cursor(CursorStyle::IBeam)
            })
            // No focus ring. A ring says *widget*, and a document is not one —
            // the caret already paints only while focused, so a box around the
            // whole page is a second, louder signal for the same fact.
            .relative()
            .child(input)
            // The document is inset by the gutter so the handle has somewhere
            // to sit *inside* the editor. Outside it the handle is clipped by
            // any scrolling ancestor, and a pointer over it never reaches
            // `on_mouse_move`, which fires only while this element is the one
            // under the pointer.
            .child(
                div()
                    .w_full()
                    .pl(gpui::px(layout.text_inset))
                    .child(match self.mode {
                        // The source is one text, so it paints as one text —
                        // the same caret, the same clicks, no block chrome to
                        // suppress a piece at a time.
                        Mode::Source => markdown::render_source(
                            self.source_text(),
                            markdown::Editing {
                                selection,
                                caret_on: self.caret_on,
                                layouts: Some(&self.layouts),
                                typography: Some(markdown::Typography::of(cx).scaled(
                                    text_size::resolve(self.text_size, cx)
                                        / theme::base_text_size(),
                                )),
                                ..Default::default()
                            },
                            cx,
                        ),
                        Mode::Blocks => markdown::render_with(
                            &self.doc,
                            markdown::Editing {
                                selection,
                                caret_on: self.caret_on,
                                layouts: Some(&self.layouts),
                                annotations: &self.annotations(),
                                placeholder: focused.then(|| PLACEHOLDER.into()),
                                // A caret goes into the caption here, so it is always
                                // painted — an editor that could hide it would be
                                // hiding a place you can already be typing.
                                caption: markdown::Caption::Shown,
                                // The size is absolute, so the factor the ladder
                                // is already scaled by comes back out of it —
                                // otherwise the app's size and this one multiply.
                                typography: Some(markdown::Typography::of(cx).scaled(
                                    text_size::resolve(self.text_size, cx)
                                        / theme::base_text_size(),
                                )),
                                // The editor's own press hit-tests
                                // `checkbox_bounds`, which is what keeps a
                                // toggle in the undo history.
                                toggle: Some(markdown::Toggle::HitTested),
                                base: self.base.as_deref(),
                                ..Default::default()
                            },
                            window,
                            cx,
                        ),
                    }),
            )
            // Last, so the layouts it reads are this frame's rather than the
            // one before — children paint in order.
            .child(
                canvas(|_, _, _| (), {
                    let entity = cx.entity();
                    move |_, _, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.reveal_caret(cx);
                            this.settle_handle(window, cx);
                        });
                    }
                })
                .absolute()
                .size(gpui::px(0.0)),
            )
            .children(self.slash_menu(&theme, cx))
            .children(self.paste_menu(&theme, cx))
            .children(self.url_prompt(&theme, cx))
            .children(self.image_target(cx))
            .children(self.resize_preview())
            .children(self.handle(focused, &theme, cx))
            .children(self.resize_handle(&theme, cx))
            .children(self.drop_indicator(&theme))
            .children(self.language_chip(&theme, cx))
            .children(self.block_menu(&theme, cx))
            .children(self.language_menu(&theme, cx))
    }
}
