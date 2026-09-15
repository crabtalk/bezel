//! What a node is: how it paints, how its box is sized and dressed, what
//! editing it changes, what a double-click opens, and what `tab` under it
//! makes.
//!
//! ```ignore
//! canvas::set_kinds(cx, Kinds::new().with_root(vault).with("session", session(store)));
//! ```
//!
//! Keyed by the node's `type`. A kind is closures, so it holds what it needs;
//! the view it paints stays with the app, looked up by the node's id. The
//! spec's four are [`text`], [`file`], [`link`] and [`group`], each
//! replaceable, and a type nothing names is [`unknown`].

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::{
    AnyElement, App, Global, Hsla, ObjectFit, Styled, StyledImage, Window, div, img, prelude::*, px,
};
use markdown::{Editing, Marks, Typography};
use theme::{TextStyle, Theme};

use crate::{
    mindmap::{NODE_HEIGHT, NODE_WIDTH},
    model::{self, Node},
};

/// How a node's box is sized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sizing {
    /// The document's width and height; content past them is clipped.
    Fixed,
    /// The document's width, as tall as the content. The height is written
    /// back.
    Grows,
}

/// The box a node is painted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    /// Filled and bordered.
    Card,
    /// Bordered only.
    Outline,
    /// Nothing: the content is the node.
    Bare,
    /// Bordered and washed in the node's colour, its content unclipped so a
    /// label can sit above the box — a group.
    Frame,
}

/// A node's content at `zoom`. See [`text_style`].
pub type Paint = Rc<dyn Fn(&Node, f32, &mut Window, &mut App) -> AnyElement>;
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
    pub chrome: Chrome,
    /// What `f2` edits, and a double-click too when the kind opens nothing.
    pub edit: Option<Field>,
    /// What a double-click does in place of editing.
    pub open: Option<Open>,
    /// The node `tab` makes under one of this kind. Its id and position are
    /// the canvas's to set.
    pub child: Child,
}

impl Kind {
    /// A fixed card painted by `render`, editing and opening nothing, with a
    /// blank text node under it.
    pub fn new(render: impl Fn(&Node, f32, &mut Window, &mut App) -> AnyElement + 'static) -> Self {
        Self {
            render: Rc::new(render),
            sizing: Sizing::Fixed,
            chrome: Chrome::Card,
            edit: None,
            open: None,
            child: Rc::new(blank),
        }
    }

    pub fn sizing(mut self, sizing: Sizing) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
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
}

impl Kinds {
    /// The spec's four.
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
        }
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

/// What `name` is, from the installed kinds.
pub fn kind(cx: &App, name: &str) -> Kind {
    match cx.try_global::<Installed>() {
        Some(installed) => installed.0.get(name).clone(),
        None => Kinds::new().get(name).clone(),
    }
}

/// Markdown, as tall as it runs, edited in place.
pub fn text() -> Kind {
    Kind::new(render_text)
        .sizing(Sizing::Grows)
        .edit(Field::new(
            |node| node.text.clone().unwrap_or_default(),
            |node, text| node.text = Some(text),
        ))
}

/// Its path. [`Kinds::with_root`] previews images.
pub fn file() -> Kind {
    file_under(None)
}

/// Its URL, which a double-click opens and `f2` edits.
pub fn link() -> Kind {
    Kind::new(|node, zoom, _, cx| {
        label(
            node.url.clone().unwrap_or_default(),
            zoom,
            Theme::of(cx).accent,
        )
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

/// A frame for the nodes inside it, its label above. [`Kinds::with_root`]
/// paints its background.
pub fn group() -> Kind {
    group_under(None)
}

/// A type no kind names: its name, faint.
pub fn unknown() -> Kind {
    Kind::new(|node, zoom, _, cx| label(node.kind.clone(), zoom, Theme::of(cx).text_faint))
}

/// Text in `style` at `zoom`: size, leading and weight together. Scaling the
/// size alone keeps the full leading, and the text spills out of its node.
pub fn text_style<E: Styled>(el: E, style: TextStyle, zoom: f32) -> E {
    el.text_size(px(style.painted() * zoom))
        .line_height(px(style.painted_line_height() * zoom))
        .font_weight(style.weight())
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
    Kind::new(move |node, zoom, _, cx| {
        let path = node.file.as_deref().unwrap_or_default();
        match resolve(root.as_deref(), path).filter(|_| is_image(path)) {
            Some(image) => img(image)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => {
                let subpath = node.subpath.as_deref().unwrap_or_default();
                label(format!("{path}{subpath}"), zoom, Theme::of(cx).text)
            }
        }
    })
}

fn group_under(root: Option<Rc<Path>>) -> Kind {
    Kind::new(move |node, zoom, _, cx| {
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
        div()
            .relative()
            .size_full()
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
            .into_any_element()
    })
    .chrome(Chrome::Frame)
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

fn render_text(node: &Node, zoom: f32, window: &mut Window, cx: &mut App) -> AnyElement {
    let doc = markdown::parse_with(node.text.as_deref().unwrap_or_default(), &Marks::of(cx));
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
