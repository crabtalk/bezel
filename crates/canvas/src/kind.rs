//! What a node is. The canvas owns a node's box — where it sits, its size, the
//! selection ring and handles, dragging and resizing — and its kind owns
//! everything painted inside: how it looks, what editing changes, what a
//! double-click opens, what `tab` under it makes, and whether it holds what is
//! dropped inside it.
//!
//! ```ignore
//! canvas::set_kinds(cx, Kinds::new().with_root(vault).with("session", session(store)));
//! ```
//!
//! Keyed by the node's `type`. A kind is closures, so it holds what it needs.
//! The spec's four are [`text`], [`file`], [`link`] and [`group`], each
//! replaceable, and a type nothing names is [`unknown`]. [`chrome`] dresses a
//! box the way they do.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::{
    AnyElement, App, Div, Global, Hsla, ObjectFit, Rgba, Styled, StyledImage, Window, div, img,
    prelude::*, px,
};
use markdown::{Doc, Editing, Marks, Typography};
use theme::{TextStyle, Theme};

use crate::{
    mindmap::{NODE_HEIGHT, NODE_WIDTH},
    model::{self, Node},
};

/// Inside a dressed box, in canvas units.
pub const PAD: f32 = 12.0;
/// A dressed box's corners, in canvas units.
pub const RADIUS: f32 = 8.0;
/// A coloured frame's wash of its colour.
const FRAME_WASH: f32 = 0.06;

/// How a node's box is sized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sizing {
    /// The document's width and height.
    Fixed,
    /// The document's width, as tall as the content. The height is written
    /// back.
    Grows,
}

/// How [`chrome`] dresses a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    /// Filled and bordered.
    Card,
    /// Bordered only.
    Outline,
    /// Nothing.
    Bare,
    /// Bordered and washed in the node's colour, a label free to sit above —
    /// a group.
    Frame,
}

/// What a node paints with.
pub struct Look {
    pub zoom: f32,
    /// The editor typing into the node while it is edited, for the kind to
    /// place where the field it edits shows.
    pub editor: Option<AnyElement>,
}

/// A node's content. See [`text_style`] and [`chrome`].
pub type Paint = Rc<dyn Fn(&Node, Look, &mut Window, &mut App) -> AnyElement>;
pub type Read = Rc<dyn Fn(&Node) -> String>;
pub type Write = Rc<dyn Fn(&mut Node, String)>;
pub type Open = Rc<dyn Fn(&Node, &mut App)>;
pub type Child = Rc<dyn Fn(&Node) -> Node>;

/// The string field editing a node in place changes.
#[derive(Clone)]
pub struct Field {
    pub read: Read,
    pub write: Write,
}

impl Field {
    pub fn new(
        read: impl Fn(&Node) -> String + 'static,
        write: impl Fn(&mut Node, String) + 'static,
    ) -> Self {
        Self {
            read: Rc::new(read),
            write: Rc::new(write),
        }
    }
}

#[derive(Clone)]
pub struct Kind {
    pub render: Paint,
    pub sizing: Sizing,
    /// What `f2` edits, and a double-click too when the kind opens nothing.
    pub edit: Option<Field>,
    /// What a double-click does in place of editing.
    pub open: Option<Open>,
    /// The node `tab` makes under one of this kind. Its id and position are
    /// the canvas's to set.
    pub child: Child,
    /// A node inside its box that names no container is held by it, and it is
    /// never a drop target. See [`crate::contain`].
    pub holds: bool,
}

impl Kind {
    /// A fixed box painted wholly by `render`, editing, opening and holding
    /// nothing, with a blank text node under it.
    pub fn new(
        render: impl Fn(&Node, Look, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            render: Rc::new(render),
            sizing: Sizing::Fixed,
            edit: None,
            open: None,
            child: Rc::new(blank),
            holds: false,
        }
    }

    /// Hold what sits inside its box, as a group does.
    pub fn holds(mut self) -> Self {
        self.holds = true;
        self
    }

    pub fn sizing(mut self, sizing: Sizing) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn edit(mut self, field: Field) -> Self {
        self.edit = Some(field);
        self
    }

    pub fn open(mut self, open: impl Fn(&Node, &mut App) + 'static) -> Self {
        self.open = Some(Rc::new(open));
        self
    }

    pub fn child(mut self, child: impl Fn(&Node) -> Node + 'static) -> Self {
        self.child = Rc::new(child);
        self
    }
}

/// Every kind a canvas paints, by `type`.
#[derive(Clone)]
pub struct Kinds {
    kinds: HashMap<String, Kind>,
    unknown: Kind,
    fresh: Rc<dyn Fn() -> Node>,
}

impl Kinds {
    /// The spec's four, and a blank text node made from nothing.
    pub fn new() -> Self {
        let kinds = [
            (model::TEXT, text()),
            (model::FILE, file()),
            (model::LINK, link()),
            (model::GROUP, group()),
        ]
        .into_iter()
        .map(|(name, kind)| (name.to_owned(), kind))
        .collect();
        Self {
            kinds,
            unknown: unknown(),
            fresh: Rc::new(|| blank(&Node::default())),
        }
    }

    /// What the canvas makes from nothing: a double-click on empty canvas,
    /// `tab` on an empty one, and pasted text, written through its kind's
    /// edit field. Its id and position are the canvas's to set.
    pub fn with_fresh(mut self, fresh: impl Fn() -> Node + 'static) -> Self {
        self.fresh = Rc::new(fresh);
        self
    }

    /// Files and group backgrounds are found under `root`, so images preview.
    /// This replaces `file` and `group`: call it before [`Kinds::with`].
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        let root: Rc<Path> = Rc::from(root.into());
        self.kinds
            .insert(model::FILE.into(), file_under(Some(root.clone())));
        self.kinds
            .insert(model::GROUP.into(), group_under(Some(root)));
        self
    }

    /// Add a kind, or replace one — the spec's included.
    pub fn with(mut self, name: impl Into<String>, kind: Kind) -> Self {
        self.kinds.insert(name.into(), kind);
        self
    }

    pub fn get(&self, name: &str) -> &Kind {
        self.kinds.get(name).unwrap_or(&self.unknown)
    }
}

impl Default for Kinds {
    fn default() -> Self {
        Self::new()
    }
}

struct Installed(Kinds);

impl Global for Installed {}

/// `canvas::set_kinds(cx, kinds)` — call once at boot.
pub fn set_kinds(cx: &mut App, kinds: Kinds) {
    cx.set_global(Installed(kinds));
}

/// The spec's kinds, unless an app set its own.
pub(crate) fn ensure(cx: &mut App) {
    if !cx.has_global::<Installed>() {
        set_kinds(cx, Kinds::new());
    }
}

fn installed<R>(cx: &App, read: impl FnOnce(&Kinds) -> R) -> R {
    match cx.try_global::<Installed>() {
        Some(installed) => read(&installed.0),
        None => read(&Kinds::new()),
    }
}

/// What `name` is, from the installed kinds.
pub fn kind(cx: &App, name: &str) -> Kind {
    installed(cx, |kinds| kinds.get(name).clone())
}

/// Whether a node of type `name` holds what sits inside it.
pub fn holds(cx: &App, name: &str) -> bool {
    installed(cx, |kinds| kinds.get(name).holds)
}

/// A node made from nothing, holding `text` when its kind edits one.
pub fn fresh(cx: &App, text: Option<String>) -> Node {
    installed(cx, |kinds| {
        let mut node = (kinds.fresh)();
        if let (Some(text), Some(field)) = (text, &kinds.get(&node.kind).edit) {
            (field.write)(&mut node, text);
        }
        node
    })
}

/// Markdown in a card, as tall as it runs, edited in place.
pub fn text() -> Kind {
    let parsed = Parsed::default();
    Kind::new(move |node, look, window, cx| {
        let content = match look.editor {
            Some(editor) => editor,
            None => render_text(&parsed, node, look.zoom, window, cx),
        };
        chrome(Chrome::Card, node, look.zoom, cx)
            .child(content)
            .into_any_element()
    })
    .sizing(Sizing::Grows)
    .edit(Field::new(
        |node| node.text.clone().unwrap_or_default(),
        |node, text| node.text = Some(text),
    ))
}

/// Its path in a card. [`Kinds::with_root`] previews images.
pub fn file() -> Kind {
    file_under(None)
}

/// Its URL in a card, which a double-click opens and `f2` edits.
pub fn link() -> Kind {
    Kind::new(|node, look, _, cx| {
        let url = label(
            node.url.clone().unwrap_or_default(),
            look.zoom,
            Theme::of(cx).accent,
        );
        chrome(Chrome::Card, node, look.zoom, cx)
            .child(look.editor.unwrap_or(url))
            .into_any_element()
    })
    .edit(Field::new(
        |node| node.url.clone().unwrap_or_default(),
        |node, url| node.url = Some(url),
    ))
    .open(|node, cx| {
        if let Some(url) = &node.url {
            cx.open_url(url);
        }
    })
}

/// A frame holding what sits inside it, its label above. [`Kinds::with_root`]
/// paints its background.
pub fn group() -> Kind {
    group_under(None)
}

/// A type no kind names: its name, faint, in a card.
pub fn unknown() -> Kind {
    Kind::new(|node, look, _, cx| {
        let name = label(node.kind.clone(), look.zoom, Theme::of(cx).text_faint);
        chrome(Chrome::Card, node, look.zoom, cx)
            .child(name)
            .into_any_element()
    })
}

/// A box dressed as `dress` that fills the node, for a kind's content. Its
/// border takes the node's colour.
pub fn chrome(dress: Chrome, node: &Node, zoom: f32, cx: &App) -> Div {
    let theme = Theme::of(cx);
    let tint = node.color.as_deref().and_then(|c| color(theme, c));
    let border = tint.unwrap_or(theme.border);
    let body = div()
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .size_full()
        .rounded(px(RADIUS * zoom));
    match dress {
        Chrome::Card => body
            .p(px(PAD * zoom))
            .border_1()
            .border_color(border)
            .bg(theme.surface_card)
            .overflow_hidden(),
        Chrome::Outline => body
            .p(px(PAD * zoom))
            .border_1()
            .border_color(border)
            .overflow_hidden(),
        Chrome::Bare => body,
        Chrome::Frame => body
            .border_1()
            .border_color(border)
            .bg(tint.map_or(gpui::transparent_black(), |c| c.opacity(FRAME_WASH))),
    }
}

/// A JSON Canvas colour: a preset, or hex. Yellow and cyan have no token, so
/// they turn the hue of the token beside them.
pub fn color(theme: &Theme, color: &str) -> Option<Hsla> {
    match color {
        "1" => Some(theme.danger),
        "2" => Some(theme.warning),
        "3" => Some(Hsla {
            h: 50.0 / 360.0,
            ..theme.warning
        }),
        "4" => Some(theme.success),
        "5" => Some(Hsla {
            h: 185.0 / 360.0,
            ..theme.success
        }),
        "6" => Some(theme.accent),
        hex => Rgba::try_from(hex).ok().map(Into::into),
    }
}

/// Text in `style` at `zoom`: size, leading and weight together. Scaling the
/// size alone keeps the full leading, and the text spills out of its node.
pub fn text_style<E: Styled>(el: E, style: TextStyle, zoom: f32) -> E {
    el.text_size(px(style.painted() * zoom))
        .line_height(px(style.painted_line_height() * zoom))
        .font_weight(style.weight())
}

/// Our own node field: the height a growing node was pulled to. Its content
/// may still run taller.
pub const MIN_HEIGHT: &str = "minHeight";

pub fn min_height(node: &Node) -> Option<i64> {
    node.extra
        .get(MIN_HEIGHT)
        .and_then(serde_json::Value::as_i64)
}

/// An empty text node of the default size.
pub fn blank(_: &Node) -> Node {
    Node {
        kind: model::TEXT.into(),
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
        text: Some(String::new()),
        ..Node::default()
    }
}

/// Whether a path names an image gpui paints.
pub fn is_image(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
            )
        })
}

fn file_under(root: Option<Rc<Path>>) -> Kind {
    Kind::new(move |node, look, _, cx| {
        let path = node.file.as_deref().unwrap_or_default();
        let content = match resolve(root.as_deref(), path).filter(|_| is_image(path)) {
            Some(image) => img(image)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => {
                let subpath = node.subpath.as_deref().unwrap_or_default();
                label(format!("{path}{subpath}"), look.zoom, Theme::of(cx).text)
            }
        };
        chrome(Chrome::Card, node, look.zoom, cx)
            .child(look.editor.unwrap_or(content))
            .into_any_element()
    })
}

fn group_under(root: Option<Rc<Path>>) -> Kind {
    Kind::new(move |node, look, _, cx| {
        let zoom = look.zoom;
        let backdrop = node
            .background
            .as_deref()
            .and_then(|path| resolve(root.as_deref(), path))
            .map(|image| {
                let fit = match node.background_style.as_deref() {
                    Some("ratio") => ObjectFit::Contain,
                    _ => ObjectFit::Cover,
                };
                img(image).size_full().object_fit(fit)
            });
        let title = node.label.clone().unwrap_or_default();
        chrome(Chrome::Frame, node, zoom, cx)
            .relative()
            .children(backdrop)
            .when(!title.is_empty(), |d| {
                d.child(
                    text_style(div(), TextStyle::Callout, zoom)
                        .absolute()
                        .left_0()
                        .bottom(px(node.height as f32 * zoom))
                        .pb(px(4.0 * zoom))
                        .text_color(Theme::of(cx).text_muted)
                        .child(title),
                )
            })
            .children(look.editor)
            .into_any_element()
    })
    .holds()
    .edit(Field::new(
        |node| node.label.clone().unwrap_or_default(),
        |node, label| node.label = Some(label),
    ))
}

/// `path` under `root`, or as it is when absolute.
fn resolve(root: Option<&Path>, path: &str) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    match root {
        Some(root) => Some(root.join(path)),
        None => Path::new(path).is_absolute().then(|| PathBuf::from(path)),
    }
}

/// Parsed text kept between frames by node id, while its source holds.
type Parsed = Rc<RefCell<HashMap<String, (String, Rc<Doc>)>>>;

/// How many parsed texts are kept before they are all let go.
const PARSED: usize = 4096;

fn render_text(
    parsed: &Parsed,
    node: &Node,
    zoom: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let source = node.text.as_deref().unwrap_or_default();
    let doc = {
        let mut parsed = parsed.borrow_mut();
        match parsed.get(&node.id) {
            Some((was, doc)) if was == source => doc.clone(),
            _ => {
                if parsed.len() >= PARSED {
                    parsed.clear();
                }
                let doc = Rc::new(markdown::parse_with(source, &Marks::of(cx)));
                parsed.insert(node.id.clone(), (source.to_owned(), doc.clone()));
                doc
            }
        }
    };
    let editing = Editing {
        typography: Some(Typography::of(cx).scaled(zoom)),
        ..Editing::default()
    };
    markdown::render_with(&doc, editing, window, cx)
}

fn label(text: String, zoom: f32, color: Hsla) -> AnyElement {
    text_style(div(), TextStyle::Callout, zoom)
        .text_color(color)
        .child(text)
        .into_any_element()
}
