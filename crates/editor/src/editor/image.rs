//! Where a picture comes from.
//!
//! Four ways in, and they differ only in what they have to go on. A pasted URL
//! and the slash menu already name a place to fetch from. A dropped file names
//! one on disk. A pasted screenshot names nothing at all — it is bytes, and
//! bytes have to be put somewhere before a document can point at them, which is
//! the app's decision and not this library's. Hence [`set_image_store`],
//! installed once at boot like `markdown::set_link_preview`.

use std::path::Path;

use gpui::{
    AnyElement, App, Context, CursorStyle, Entity, ExternalPaths, Focusable as _, Global,
    MouseButton, Point, Window, div, prelude::*, px,
};
use markdown::{Block, BlockKind, Cursor, Part, Selection, Text};
use theme::Theme;
use ui::{input::TextField, popover};

use crate::{
    editor::{
        Editor,
        keys::{CancelUrl, ConfirmUrl},
    },
    history::EditKind,
};

/// The key context the URL prompt claims, so `enter` fills the picture in
/// rather than splitting the document behind it.
pub const PROMPT_CONTEXT: &str = "BezelImagePrompt";

/// Wide enough for a URL to be read back before it is confirmed.
const PROMPT_WIDTH: f32 = 280.0;

/// A picture with no address of its own, and what the app is asked about.
pub enum Source<'a> {
    /// Bytes off the clipboard — a screenshot, which has no name anywhere.
    Bytes(&'a gpui::Image),
    /// A file dragged in from outside, which has one until it is moved.
    File(&'a Path),
}

/// The URL to write down for `source`, and `None` for one the app will not
/// keep — a dropped file then stays where it already is, and a screenshot is
/// let go.
pub type ImageStore = fn(Source) -> Option<String>;

struct Installed(ImageStore);

impl Global for Installed {}

/// `editor::set_image_store(cx, my_store)` — call once at boot. Without it a
/// screenshot cannot be pasted at all: there is nowhere to put the bytes, and
/// a document holds a URL.
pub fn set_image_store(cx: &mut App, store: ImageStore) {
    cx.set_global(Installed(store));
}

fn stored(cx: &App, source: Source) -> Option<String> {
    (cx.try_global::<Installed>()?.0)(source)
}

/// An open prompt: the image it will fill, the field being typed into, and
/// where the card hangs.
pub(crate) struct Prompt {
    block: usize,
    field: Entity<TextField>,
    at: Point<gpui::Pixels>,
}

impl Editor {
    /// Ask for the URL of the image at `ix`, and hand it the keys.
    pub(super) fn prompt_for_url(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.layouts.block_bounds(ix) else {
            return;
        };
        // The context goes on the *field*: gpui resolves a chord to the binding
        // matching deepest in the focus path, and nothing is deeper than the
        // focused field — a container around it cannot win `enter`.
        let field = cx.new(|cx| {
            TextField::new(cx)
                .with_placeholder("Paste an image URL")
                .with_key_context(PROMPT_CONTEXT)
        });
        window.focus(&field.focus_handle(cx), cx);
        self.slash = None;
        self.url_prompt = Some(Prompt {
            block: ix,
            field,
            at: gpui::point(
                bounds.origin.x - self.origin.x,
                bounds.origin.y - self.origin.y + bounds.size.height,
            ),
        });
        cx.notify();
    }

    /// Take what was typed. An image already carries its caption, and
    /// [`markdown::Doc::set_kind`] carries it across the turn.
    pub(super) fn confirm_url(
        &mut self,
        _: &ConfirmUrl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.url_prompt.take() else {
            return;
        };
        let url = prompt.field.read(cx).content().trim().to_string();
        self.focus_handle.clone().focus(window, cx);
        if url.is_empty() {
            return cx.notify();
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(
                prompt.block,
                BlockKind::Image {
                    url,
                    alt: Text::default(),
                },
            );
            this.selection =
                Selection::at(Cursor::new(prompt.block, Part::Caption, 0).clamp(&this.doc));
        });
    }

    pub(super) fn cancel_url(
        &mut self,
        _: &CancelUrl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.url_prompt.take().is_some() {
            self.focus_handle.clone().focus(window, cx);
            cx.notify();
        }
    }

    /// Files dragged in from outside. The store says where each one belongs,
    /// and a picture it does not want paints from where it already is.
    pub(super) fn drop_paths(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let urls: Vec<String> = paths
            .paths()
            .iter()
            .filter(|path| markdown::is_image(&path.to_string_lossy()))
            .map(|path| {
                stored(cx, Source::File(path))
                    .unwrap_or_else(|| path.to_string_lossy().into_owned())
            })
            .collect();
        let ix = self.dropping.take().unwrap_or(self.cursor().block);
        self.place_images(ix, urls, cx);
        cx.notify();
    }

    /// Bytes off the clipboard. `false` when no store is installed, which
    /// leaves the paste to whatever else the clipboard was carrying.
    pub(super) fn paste_image(&mut self, image: &gpui::Image, cx: &mut Context<Self>) -> bool {
        let Some(url) = stored(cx, Source::Bytes(image)) else {
            return false;
        };
        self.place_images(self.cursor().block, vec![url], cx);
        true
    }

    /// Put pictures into the document after block `ix` — or in its place, when
    /// that block is the empty paragraph a caret was left sitting in.
    fn place_images(&mut self, ix: usize, urls: Vec<String>, cx: &mut Context<Self>) {
        if urls.is_empty() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let here = this.doc.blocks.get(ix);
            let indent = here.map_or(0, |block| block.indent);
            let empty = here.is_some_and(
                |block| matches!(&block.kind, BlockKind::Paragraph(text) if text.is_empty()),
            );
            let first = if empty {
                this.doc.blocks.remove(ix);
                ix
            } else {
                (ix + 1).min(this.doc.blocks.len())
            };
            let last = first + urls.len() - 1;
            for (offset, url) in urls.into_iter().enumerate() {
                this.doc.blocks.insert(
                    first + offset,
                    Block::at(
                        BlockKind::Image {
                            url,
                            alt: Text::default(),
                        },
                        indent,
                    ),
                );
            }
            this.doc.repair();
            this.selection = Selection::at(Cursor::new(last, Part::Caption, 0).clamp(&this.doc));
        });
    }

    /// The click target over an image with no URL yet — the row that says so
    /// is painted by the renderer, and this is what answers a press on it.
    ///
    /// The one under the pointer, else the one the caret is in, which is
    /// [`Editor::language_chip`]'s rule and for the same reason: one element
    /// placed from the recorded frames, rather than one per block.
    pub(super) fn image_target(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let blank = |ix: &usize| {
            matches!(
                self.doc.blocks.get(*ix).map(|block| &block.kind),
                Some(BlockKind::Image { url, .. }) if url.is_empty()
            )
        };
        let ix = [self.hovered, Some(self.cursor().block)]
            .into_iter()
            .flatten()
            .find(blank)?;
        let bounds = self.layouts.block_bounds(ix)?;
        Some(
            div()
                .id("image-target")
                .absolute()
                .left(bounds.origin.x - self.origin.x)
                .top(bounds.origin.y - self.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &gpui::MouseDownEvent, window, cx| {
                        // The editor's own press listener runs after this one
                        // and would take the focus straight back.
                        this.press_claimed = true;
                        this.prompt_for_url(ix, window, cx);
                    }),
                )
                .into_any_element(),
        )
    }

    /// The prompt itself, under the picture it belongs to.
    pub(super) fn url_prompt(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.url_prompt.as_ref()?;
        Some(popover::menu_at(
            "image-url",
            prompt.at,
            popover::popover_card(theme)
                .w(px(PROMPT_WIDTH))
                // Without this the editor's own press listener runs next and
                // takes the focus back off the field being clicked into.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &gpui::MouseDownEvent, _, _| this.press_claimed = true),
                )
                .on_mouse_down_out(
                    cx.listener(|this, _, window, cx| this.cancel_url(&CancelUrl, window, cx)),
                )
                .child(popover::search_input_frame(
                    theme,
                    prompt.field.clone().into_any_element(),
                ))
                .into_any_element(),
            None,
        ))
    }
}
