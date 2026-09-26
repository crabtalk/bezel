use crate::*;

impl Gallery {
    pub(crate) fn data(
        &mut self,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let view = Painter::of(cx);
        let section = stack();

        Some(match key {
            "alerts" => section
                .child(theme.error_strip("Something went wrong."))
                .child(theme.warning_strip("Heads up, check this."))
                .into_any_element(),

            "step-row" => {
                let card = |index: usize, first: bool| {
                    let Step {
                        icon,
                        title,
                        detail,
                        meta,
                        failed,
                        output,
                    } = STEPS[index];
                    let open = self.data.step_open[index];
                    div()
                        .when(!first, |el| el.border_t_1().border_color(theme.border))
                        .child(
                            theme
                                .step_row(
                                    icon,
                                    title,
                                    Some(SharedString::from(detail)),
                                    Some(SharedString::from(meta)),
                                    failed,
                                    output.map(|_| open),
                                )
                                .hover(|s| s.bg(theme.element_hover))
                                .id(SharedString::from(format!("step-{index}")))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.data.step_open[index] = !view.data.step_open[index];
                                    cx.notify();
                                })),
                        )
                        .when_some(output.filter(|_| open), |el, output| {
                            el.child(theme.step_output(
                                SharedString::from(format!("step-out-{index}")),
                                output,
                            ))
                        })
                };

                section
                    .child(hint(
                        &theme,
                        "An operation with an outcome: press a row to see what it \
                         printed. A step with no output has no chevron — a \
                         disclosure onto nothing is worse than none.",
                    ))
                    .child(
                        // Standalone: one step in its own box.
                        div()
                            .w_full()
                            .max_w(px(420.0))
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(card(0, true)),
                    )
                    .child(hint(
                        &theme,
                        "Or as a run: the same rows, borderless, in one box that \
                         owns the hairlines between them.",
                    ))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(420.0))
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(card(1, true))
                            .child(card(2, false)),
                    )
                    .into_any_element()
            }

            "skeleton" => section
                .child(popover::redacted_rows("g-redacted", &theme, 3, view, cx))
                .into_any_element(),

            "textarea" => section
                .child(hint(
                    &theme,
                    "The same TextField under a different Shape — enter breaks a line, \
                     up/down and ctrl-p/ctrl-n walk rows keeping their column, and \
                     ctrl-a/ctrl-e go to the ends of the logical line rather than \
                     stopping at a wrap.",
                ))
                .child(shape_demo(
                    &theme,
                    "Shape::Rows(4)",
                    self.data.notes.clone(),
                ))
                .child(shape_demo(
                    &theme,
                    "Shape::Grow { min: 2, max: 6 }",
                    self.data.composer.clone(),
                ))
                .child(hint(
                    &theme,
                    "Past the last row it scrolls: the caret is kept in view as you \
                     type or move, and the wheel scrolls away from it without being \
                     dragged back.",
                ))
                .into_any_element(),

            // ---- Not built yet -----------------------------------------------
            "date-picker" => section
                .child(hint(
                    &theme,
                    "enter opens it. The arrows walk days and weeks — off the end \
                     of a month and the grid follows — pageup and pagedown page \
                     months, enter chooses, escape dismisses.",
                ))
                .child(div().w(px(220.0)).child(self.data.date.clone()))
                .child(
                    div()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(SharedString::from(
                            match self.data.date.read(cx).selection() {
                                Some(date) => format!("chosen: {date}"),
                                None => "nothing chosen".to_string(),
                            },
                        )),
                )
                .into_any_element(),

            "menubar" => section
                .child(hint(
                    &theme,
                    "Open one, then slide across the others — a bar with a menu \
                     down switches on hover, with no second click. A row with a \
                     chevron drops a menu of its own, on hover or on `right`; \
                     `left` and `escape` close one level at a time. The greyed \
                     rows cannot be landed on at all.",
                ))
                .child(self.menubar.clone())
                .child(
                    div()
                        .text_style(TextStyle::Callout)
                        .text_color(theme.text_muted)
                        .child(SharedString::from(match &self.last_menu_item {
                            Some(label) => format!("chose: {label}"),
                            None => "nothing chosen".to_string(),
                        })),
                )
                .into_any_element(),

            "pagination" => {
                section
                    .child(hint(
                        &theme,
                        "For data that arrives in pages — a result set the client \
                     cannot hold — and not for lists that are merely long: those \
                     are the scroll area and the virtualized list. Walk to either \
                     end and the run of pages keeps its width.",
                    ))
                    .child(
                        pagination::pagination()
                            .child(
                                div()
                                    .id("page-prev")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.go_to_page(view.data.page.saturating_sub(1), cx)
                                    }))
                                    .child(pagination::step(
                                        &theme,
                                        icons::glyph::ChevronLeft,
                                        self.data.page > 1,
                                    )),
                            )
                            .children(
                                pagination::window(self.data.page, RESULT_PAGES, 2)
                                    .into_iter()
                                    .enumerate()
                                    .map(|(slot, entry)| match entry {
                                        pagination::Slot::Gap => {
                                            pagination::ellipsis(&theme).into_any_element()
                                        }
                                        pagination::Slot::Page(page) => pagination::page_button(
                                            &theme,
                                            page,
                                            page == self.data.page,
                                        )
                                        .id(SharedString::from(format!("page-{slot}")))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.go_to_page(page, cx)
                                        }))
                                        .into_any_element(),
                                    }),
                            )
                            .child(
                                div()
                                    .id("page-next")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.go_to_page(view.data.page + 1, cx)
                                    }))
                                    .child(pagination::step(
                                        &theme,
                                        icons::glyph::ChevronRight,
                                        self.data.page < RESULT_PAGES,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_style(TextStyle::Callout)
                            .text_color(theme.text_muted)
                            .child(SharedString::from(format!(
                                "page {} of {RESULT_PAGES}",
                                self.data.page
                            ))),
                    )
                    .into_any_element()
            }

            "scroll-area" => section
                .child(hint(
                    &theme,
                    "Drag the thumb, or use the wheel over it — the bar overlays \
                     the content rather than taking a gutter, so it never reflows \
                     what it sits on. The rail and this pane have one too.",
                ))
                .child(
                    div()
                        .relative()
                        .h(px(220.0))
                        .w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .child(scroll::claim_wheel(
                            scroll::pane("scroll-demo", Axes::Vertical)
                                .size_full()
                                .track_scroll(&self.data.demo_scroll)
                                .child(div().p(px(14.0)).flex().flex_col().gap(px(8.0)).children(
                                    (1..=30).map(|line| {
                                        div()
                                            .text_style(TextStyle::Callout)
                                            .text_color(theme.text_muted)
                                            .child(SharedString::from(format!("Line {line}")))
                                    }),
                                )),
                            &self.data.demo_scroll,
                            Axes::Vertical,
                            &self.data.demo_claim,
                        ))
                        .child(scroll::scrollbar(
                            "scroll-demo-bar",
                            &self.data.demo_scroll,
                            &self.data.demo_bar,
                        )),
                )
                .into_any_element(),

            "follow" => section
                .child(hint(
                    &theme,
                    "Append a line and the box stays on the newest one. Scroll up \
                     and it lets go — scroll back to the bottom and it takes over \
                     again. Neither is an event it subscribes to: the overflow \
                     changing is what tells appended content apart from you.",
                ))
                .child(
                    row()
                        .child(
                            div()
                                .id("follow-append")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.data.log_lines += 1;
                                    cx.notify();
                                }))
                                .child(theme.button(
                                    "Append a line",
                                    ButtonStyle::Ghost,
                                    Some(Fade::new(view, "g-follow-add")),
                                )),
                        )
                        .child(
                            div()
                                .id("follow-jump")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.data.log_follow.follow();
                                    cx.notify();
                                }))
                                .child(theme.button(
                                    "Jump to latest",
                                    ButtonStyle::Ghost,
                                    Some(Fade::new(view, "g-follow-pin")),
                                )),
                        )
                        // The state, on the page — the same trick the virtualized
                        // list uses for its built count. A behaviour you can only
                        // infer is a behaviour nobody can check.
                        .child(
                            div()
                                .text_style(TextStyle::Callout)
                                .font_family(theme.font_mono.clone())
                                .text_color(if self.data.log_follow.following() {
                                    theme.success
                                } else {
                                    theme.text_faint
                                })
                                .child(SharedString::from(format!(
                                    "following: {}",
                                    self.data.log_follow.following()
                                ))),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .h(px(180.0))
                        .w_full()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .child(
                            scroll::pane("follow-demo", Axes::Vertical)
                                .size_full()
                                .track_scroll(&self.data.log_scroll)
                                .child(div().p(px(14.0)).flex().flex_col().gap(px(6.0)).children(
                                    (1..=self.data.log_lines).map(|line| {
                                        div()
                                            .text_style(TextStyle::Callout)
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_muted)
                                            .child(SharedString::from(format!(
                                                "[{line:04}] token stream line {line}"
                                            )))
                                    }),
                                )),
                        )
                        .child(scroll::follow(&self.data.log_scroll, &self.data.log_follow))
                        .child(scroll::scrollbar(
                            "follow-demo-bar",
                            &self.data.log_scroll,
                            &self.data.log_bar,
                        )),
                )
                .into_any_element(),

            "drift" => {
                let accent = theme.accent;
                let chips: Vec<AnyElement> = self
                    .data
                    .drift_chips
                    .iter()
                    .map(|label| {
                        let (held, before) = (label.clone(), label.clone());
                        div()
                            .id(SharedString::from(format!("drift-chip-{label}")))
                            .flex_none()
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(Theme::control_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface_raised)
                            .text_style(TextStyle::Callout)
                            .text_color(theme.text_muted)
                            .child(label.clone())
                            .on_drag(ChipDrag(held), |drag, _, _, cx| {
                                let label = drag.0.clone();
                                cx.new(|_| HeldChip(label))
                            })
                            // Where it would land, marked on the chip it would
                            // land in front of.
                            .drag_over::<ChipDrag>(move |style, _, _, _| style.border_color(accent))
                            .on_drop(cx.listener(move |view, drag: &ChipDrag, _, cx| {
                                view.move_chip(&drag.0, &before, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();
                section
                    .child(hint(
                        &theme,
                        "Pick up a chip and carry it to either end of the strip: the strip \
                         keeps coming for as long as you hold it there, faster the closer \
                         to the edge, and past the edge is full speed. Let go over a chip \
                         to drop in front of it. Without this a strip is only as wide as \
                         the window — reaching the far end would mean letting go.",
                    ))
                    .child(
                        div()
                            .relative()
                            .h(px(76.0))
                            .w_full()
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(
                                scroll::pane("drift-demo", Axes::Horizontal)
                                    .size_full()
                                    .p(px(14.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .track_scroll(&self.data.drift_scroll)
                                    // Typed, so a drag of anything else leaves
                                    // the strip where it is.
                                    .on_drag_move(cx.listener(
                                        |view, event: &DragMoveEvent<ChipDrag>, _, _| {
                                            view.data.drift.aim(event.event.position);
                                        },
                                    ))
                                    .children(chips),
                            )
                            .child(scroll::drift(
                                &self.data.drift_scroll,
                                &self.data.drift,
                                Axes::Horizontal,
                                // The page past the strip takes no chips, so a
                                // pointer carried out there is still aiming here.
                                scroll::Beyond::Nothing,
                            )),
                    )
                    .into_any_element()
            }

            "table" => {
                let columns = table_columns();
                let mut rows = TABLE_ROWS;
                if let Some(sort) = self.data.table_sort {
                    // bezel never sees the rows: it says what the click meant
                    // and paints the arrow, and the sorting happens here.
                    rows.sort_by(|left, right| {
                        let order = match sort.column {
                            0 => left.0.cmp(right.0),
                            1 => left.1.cmp(right.1),
                            _ => left.2.cmp(&right.2),
                        };
                        if sort.ascending {
                            order
                        } else {
                            order.reverse()
                        }
                    });
                }
                section
                    .child(hint(
                        &theme,
                        "Click a heading to sort, and again to reverse it. The \
                         header sits outside the scroll container, so it stays \
                         put while the body moves under it.",
                    ))
                    .child(
                        table::table(&theme)
                            .child(
                                table::header(&theme).children(columns.iter().enumerate().map(
                                    |(index, column)| {
                                        let sorted = self
                                            .data
                                            .table_sort
                                            .filter(|sort| sort.column == index)
                                            .map(|sort| sort.ascending);
                                        table::header_cell(&theme, column, sorted)
                                            .id(SharedString::from(format!("column-{index}")))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.sort_table(index, cx)
                                            }))
                                            .into_any_element()
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h(px(150.0))
                                    .child(scroll::claim_wheel(
                                        scroll::pane("table-body", Axes::Vertical)
                                            .size_full()
                                            .track_scroll(&self.data.table_scroll)
                                            .children(rows.iter().enumerate().map(
                                                |(index, (name, kind, size))| {
                                                    table::row(
                                                        &theme,
                                                        &columns,
                                                        index == 0,
                                                        false,
                                                        vec![
                                                            SharedString::from(*name)
                                                                .into_any_element(),
                                                            div()
                                                                .text_color(theme.text_muted)
                                                                .child(SharedString::from(*kind))
                                                                .into_any_element(),
                                                            div()
                                                                .font_family(
                                                                    theme.font_mono.clone(),
                                                                )
                                                                .text_color(theme.text_muted)
                                                                .child(SharedString::from(
                                                                    format_size(*size),
                                                                ))
                                                                .into_any_element(),
                                                        ],
                                                    )
                                                },
                                            )),
                                        &self.data.table_scroll,
                                        Axes::Vertical,
                                        &self.data.table_claim,
                                    ))
                                    .child(scroll::scrollbar(
                                        "table-bar",
                                        &self.data.table_scroll,
                                        &self.data.table_bar,
                                    )),
                            ),
                    )
                    .into_any_element()
            }

            "tree" => {
                let rows = self.tree_rows();
                section
                    .child(hint(
                        &theme,
                        "Click a folder to open it, a file to choose it. The \
                         arrows walk the same rows: right opens a folder or \
                         steps into it, left closes it or leaves for its parent.",
                    ))
                    .child(
                        div()
                            .key_context(tree::KEY_CONTEXT)
                            .track_focus(&self.data.tree_focus)
                            .on_action(cx.listener(|view, _: &tree::SelectPrevious, _, cx| {
                                view.tree_step(Direction::Up, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::SelectNext, _, cx| {
                                view.tree_step(Direction::Down, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::Collapse, _, cx| {
                                view.tree_step(Direction::Left, cx)
                            }))
                            .on_action(cx.listener(|view, _: &tree::Expand, _, cx| {
                                view.tree_step(Direction::Right, cx)
                            }))
                            .relative()
                            .h(px(200.0))
                            .w_full()
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(scroll::claim_wheel(
                                scroll::pane("tree-body", Axes::Vertical)
                                    .size_full()
                                    .track_scroll(&self.data.tree_scroll)
                                    .child(tree::tree().p(px(6.0)).children(
                                        rows.iter().enumerate().map(|(index, entry)| {
                                            tree::tree_row(
                                                &theme,
                                                &entry.row,
                                                self.data.tree_selected.as_deref()
                                                    == Some(&entry.path),
                                                index == self.data.tree_cursor,
                                            )
                                            .id(SharedString::from(format!("tree-{index}")))
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.tree_click(index, window, cx)
                                            }))
                                            .child(SharedString::from(entry.label))
                                        }),
                                    )),
                                &self.data.tree_scroll,
                                Axes::Vertical,
                                &self.data.tree_claim,
                            ))
                            .child(scroll::scrollbar(
                                "tree-bar",
                                &self.data.tree_scroll,
                                &self.data.tree_bar,
                            )),
                    )
                    .when_some(self.data.tree_selected.clone(), |page, path| {
                        page.child(
                            div()
                                .text_style(TextStyle::Callout)
                                .text_color(theme.text_muted)
                                .child(SharedString::from(format!("chosen: {path}"))),
                        )
                    })
                    .into_any_element()
            }

            "virtual-list" => {
                let built = self.data.rows_built.clone();
                let muted = theme.text_muted;
                let mono = theme.font_mono.clone();
                section
                    .child(hint(
                        &theme,
                        "Ten thousand rows. The count below is how many of them \
                         the list actually built for the frame you are looking \
                         at — scroll it and it stays about the same.",
                    ))
                    .child(
                        div()
                            .relative()
                            .h(px(220.0))
                            .w_full()
                            .rounded(px(Theme::panel_radius()))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(list::virtual_list(
                                "virtual-rows",
                                VIRTUAL_ROWS,
                                px(26.0),
                                &self.data.rows_scroll,
                                move |range, _, _| {
                                    built.set(range.len());
                                    range
                                        .map(|index| {
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(10.0))
                                                .px(px(12.0))
                                                .text_style(TextStyle::Callout)
                                                .child(
                                                    div()
                                                        .w(px(56.0))
                                                        .flex_none()
                                                        .font_family(mono.clone())
                                                        .text_color(muted)
                                                        .child(SharedString::from(format!(
                                                            "{index:05}"
                                                        ))),
                                                )
                                                .child(SharedString::from(format!("Row {index}")))
                                        })
                                        .collect::<Vec<_>>()
                                },
                            ))
                            .child(scroll::scrollbar(
                                "virtual-bar",
                                &list::scroll_handle(&self.data.rows_scroll),
                                &self.data.rows_bar,
                            )),
                    )
                    .child(
                        div()
                            .text_style(TextStyle::Callout)
                            .font_family(theme.font_mono.clone())
                            .text_color(theme.text_muted)
                            .child(SharedString::from(format!(
                                "{VIRTUAL_ROWS} rows · {} built this frame",
                                self.data.rows_built.get()
                            ))),
                    )
                    .into_any_element()
            }

            _ => return None,
        })
    }
}

impl Gallery {
    /// The visible rows, rebuilt from this view's own tree and its own set of
    /// open folders.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        flatten_tree(FILE_TREE, 0, "", &self.data.tree_expanded, &mut rows);
        rows
    }

    /// An arrow key. `tree::step` decides what it meant; applying it is this
    /// view's job, because the set of open folders is this view's.
    pub(crate) fn tree_step(&mut self, direction: Direction, cx: &mut Context<Self>) {
        let rows = self.tree_rows();
        let shape: Vec<tree::Row> = rows.iter().map(|entry| entry.row).collect();
        match tree::step(&shape, self.data.tree_cursor, direction) {
            Some(Move::To(index)) => self.data.tree_cursor = index,
            Some(Move::Expand(index)) => {
                self.data.tree_expanded.insert(rows[index].path.clone());
            }
            Some(Move::Collapse(index)) => {
                self.data.tree_expanded.remove(&rows[index].path);
            }
            None => {}
        }
        cx.notify();
    }

    /// A click: a folder opens or closes, a file is chosen. Both move the
    /// keyboard cursor, so the two ways of getting around agree on where you
    /// are.
    pub(crate) fn tree_click(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.tree_rows();
        let Some(entry) = rows.get(index) else { return };
        self.data.tree_cursor = index;
        if entry.row.expanded.is_some() {
            if !self.data.tree_expanded.remove(&entry.path) {
                self.data.tree_expanded.insert(entry.path.clone());
            }
        } else {
            self.data.tree_selected = Some(entry.path.clone());
        }
        window.focus(&self.data.tree_focus, cx);
        cx.notify();
    }

    /// Go to a page, clamped the way the component clamps what it draws — the
    /// prev/next steps hand this `page - 1` and `page + 1` without checking.
    pub(crate) fn go_to_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.data.page = page.clamp(1, RESULT_PAGES);
        cx.notify();
    }

    /// A heading was clicked. `next_sort` says what that means; sorting the
    /// rows is this view's job, since they are this view's rows.
    pub(crate) fn sort_table(&mut self, column: usize, cx: &mut Context<Self>) {
        self.data.table_sort = Some(table::next_sort(self.data.table_sort, column));
        cx.notify();
    }
}

impl Gallery {
    /// Put `held` in front of `before` — where the drift demo's drop lands.
    /// The list is the app's, the way a board's cards are: bezel reports where
    /// the pointer let go and arranges nothing itself.
    pub(crate) fn move_chip(
        &mut self,
        held: &SharedString,
        before: &SharedString,
        cx: &mut Context<Self>,
    ) {
        if held == before {
            return;
        }
        let Some(from) = self.data.drift_chips.iter().position(|chip| chip == held) else {
            return;
        };
        let chip = self.data.drift_chips.remove(from);
        let at = self
            .data
            .drift_chips
            .iter()
            .position(|chip| chip == before)
            .unwrap_or(self.data.drift_chips.len());
        self.data.drift_chips.insert(at, chip);
        cx.notify();
    }
}

/// What this group's demos hold between frames.
pub(crate) struct State {
    /// The two multi-line shapes. Their row counts are this page's example, not
    /// a default the library holds — `Shape` takes them from the caller.
    pub(crate) notes: Entity<TextField>,
    pub(crate) composer: Entity<TextField>,
    /// Which step rows are showing their output.
    pub(crate) step_open: [bool; 3],
    /// So does the date picker, which holds a month and a cursor.
    pub(crate) date: Entity<Calendar>,
    pub(crate) demo_scroll: gpui::ScrollHandle,
    /// A pane nested in `gallery-pane` keeps the wheel it can act on, so
    /// scrolling it does not drag the page behind it. One per pane: the state
    /// is where that pane stood before the wheel being dispatched.
    pub(crate) demo_claim: scroll::ClaimState,
    pub(crate) demo_bar: ScrollbarState,
    /// The follow-scroll demo: a log that grows under a view pinned to its end.
    pub(crate) log_scroll: gpui::ScrollHandle,
    pub(crate) log_bar: ScrollbarState,
    pub(crate) log_follow: scroll::FollowState,
    pub(crate) log_lines: usize,
    /// The drift demo: a strip too wide for the pane, and chips to carry
    /// across it.
    pub(crate) drift_scroll: gpui::ScrollHandle,
    pub(crate) drift: scroll::DriftState,
    pub(crate) drift_chips: Vec<SharedString>,
    pub(crate) table_scroll: gpui::ScrollHandle,
    pub(crate) table_claim: scroll::ClaimState,
    pub(crate) table_bar: ScrollbarState,
    pub(crate) tree_scroll: gpui::ScrollHandle,
    pub(crate) tree_claim: scroll::ClaimState,
    pub(crate) tree_bar: ScrollbarState,
    pub(crate) rows_scroll: gpui::UniformListScrollHandle,
    pub(crate) rows_bar: ScrollbarState,
    /// How many of [`VIRTUAL_ROWS`] rows the list actually built last frame.
    /// A `Cell` because the count is written from inside the render closure,
    /// which the list owns and calls with no view in scope — and it is the only
    /// honest way to *show* that virtualization is happening.
    pub(crate) rows_built: Rc<Cell<usize>>,
    /// Which folders are open, by path. App data, and the reason `tree` reports
    /// an intent rather than expanding anything itself.
    pub(crate) tree_expanded: HashSet<String>,
    pub(crate) tree_selected: Option<String>,
    pub(crate) tree_cursor: usize,
    pub(crate) tree_focus: gpui::FocusHandle,
    /// Which page of the imaginary result set is showing. 1-based, like the
    /// component: it is a label, not an index.
    pub(crate) page: usize,
    /// Which column the table page is sorted by. The app's, because the app is
    /// what has to sort the rows — the table only says what a click meant.
    pub(crate) table_sort: Option<Sort>,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        Self {
            notes: cx.new(|cx| {
                let mut field = TextField::new(cx).with_shape(Shape::Rows(4));
                field.set_content(
                    "Wrapping is the point: this line is longer than the box, so it \
                 folds. Press enter for a hard break.",
                    cx,
                );
                field
            }),
            composer: cx.new(|cx| {
                TextField::new(cx)
                    .with_shape(Shape::Grow { min: 2, max: 6 })
                    .with_placeholder("Grows as you type…")
            }),
            step_open: [false; 3],
            date: cx.new(|cx| Calendar::new(today(), cx)),
            demo_scroll: gpui::ScrollHandle::new(),
            demo_claim: scroll::ClaimState::new(),
            demo_bar: ScrollbarState::new(Painter::of(cx)),
            log_scroll: gpui::ScrollHandle::new(),
            log_bar: ScrollbarState::new(Painter::of(cx)),
            log_follow: scroll::FollowState::new(),
            // Enough to overflow the box on arrival, so the pin has something
            // to hold onto before you press anything.
            log_lines: 24,
            drift_scroll: gpui::ScrollHandle::new(),
            drift: scroll::DriftState::new(),
            drift_chips: DRIFT_CHIPS
                .iter()
                .copied()
                .map(SharedString::from)
                .collect(),
            table_scroll: gpui::ScrollHandle::new(),
            table_claim: scroll::ClaimState::new(),
            table_bar: ScrollbarState::new(Painter::of(cx)),
            tree_scroll: gpui::ScrollHandle::new(),
            tree_claim: scroll::ClaimState::new(),
            tree_bar: ScrollbarState::new(Painter::of(cx)),
            rows_scroll: gpui::UniformListScrollHandle::new(),
            rows_bar: ScrollbarState::new(Painter::of(cx)),
            rows_built: Rc::new(Cell::new(0)),
            // Opened so the page shows nesting on arrival rather than a flat
            // list of two folders.
            tree_expanded: ["crates", "crates/ui"]
                .into_iter()
                .map(String::from)
                .collect(),
            tree_selected: None,
            tree_cursor: 0,
            tree_focus: cx.focus_handle().tab_stop(true),
            page: 1,
            table_sort: None,
        }
    }
}
