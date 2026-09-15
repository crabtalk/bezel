//! What a node is: how it paints, how its box is sized and dressed, what
//! editing it changes, and what `tab` under it makes.
//!
//! ```ignore
//! canvas::set_kinds(cx, Kinds::new().with("session", SESSION));  // once, at boot
//! ```
//!
//! Keyed by the node's `type`. State stays with the app: a `render` looks its
//! view up by the node's id and hands the entity back. The spec's four are
//! [`TEXT`], [`FILE`], [`LINK`] and [`GROUP`], each replaceable, and a type
//! nothing names is [`UNKNOWN`].

use std::collections::HashMap;

use gpui::{AnyElement, App, Global, Hsla, Styled, Window, div, prelude::*, px};
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
    /// Bordered only, for a frame other nodes sit in.
    Outline,
    /// Nothing: the content is the node.
    Bare,
}

/// The string field editing a node in place changes.
#[derive(Clone, Copy)]
pub struct Field {
    pub read: fn(&Node) -> String,
    pub write: fn(&mut Node, String),
}

#[derive(Clone, Copy)]
pub struct Kind {
    /// The content, at `zoom`. See [`text_style`].
    pub render: fn(node: &Node, zoom: f32, &mut Window, &mut App) -> AnyElement,
    pub sizing: Sizing,
    pub chrome: Chrome,
    /// What a double-click and `f2` edit, if anything.
    pub edit: Option<Field>,
    /// The node `tab` makes under one of this kind. Its id and position are
    /// the canvas's to set.
    pub child: fn(parent: &Node) -> Node,
}

pub const TEXT: Kind = Kind {
    render: render_text,
    sizing: Sizing::Grows,
    chrome: Chrome::Card,
    edit: Some(Field {
        read: read_text,
        write: write_text,
    }),
    child: blank,
};

pub const FILE: Kind = Kind {
    render: render_file,
    sizing: Sizing::Fixed,
    chrome: Chrome::Card,
    edit: None,
    child: blank,
};

pub const LINK: Kind = Kind {
    render: render_link,
    sizing: Sizing::Fixed,
    chrome: Chrome::Card,
    edit: None,
    child: blank,
};

pub const GROUP: Kind = Kind {
    render: render_group,
    sizing: Sizing::Fixed,
    chrome: Chrome::Outline,
    edit: Some(Field {
        read: read_label,
        write: write_label,
    }),
    child: blank,
};

/// A type no kind names: its name, faint.
pub const UNKNOWN: Kind = Kind {
    render: render_unknown,
    sizing: Sizing::Fixed,
    chrome: Chrome::Card,
    edit: None,
    child: blank,
};

/// Every kind a canvas paints, by `type`.
#[derive(Clone)]
pub struct Kinds(HashMap<String, Kind>);

impl Kinds {
    /// The spec's four.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Add a kind, or replace one — the spec's included.
    pub fn with(mut self, name: impl Into<String>, kind: Kind) -> Self {
        self.0.insert(name.into(), kind);
        self
    }

    pub fn get(&self, name: &str) -> Kind {
        self.0.get(name).copied().unwrap_or_else(|| builtin(name))
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

/// What `name` is, from the installed kinds.
pub fn kind(cx: &App, name: &str) -> Kind {
    cx.try_global::<Installed>()
        .map_or_else(|| builtin(name), |installed| installed.0.get(name))
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

fn builtin(name: &str) -> Kind {
    match name {
        model::TEXT => TEXT,
        model::FILE => FILE,
        model::LINK => LINK,
        model::GROUP => GROUP,
        _ => UNKNOWN,
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

fn render_file(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> AnyElement {
    let path = format!(
        "{}{}",
        node.file.as_deref().unwrap_or_default(),
        node.subpath.as_deref().unwrap_or_default()
    );
    label(path, zoom, Theme::of(cx).text)
}

fn render_link(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> AnyElement {
    label(
        node.url.clone().unwrap_or_default(),
        zoom,
        Theme::of(cx).accent,
    )
}

fn render_group(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> AnyElement {
    label(read_label(node), zoom, Theme::of(cx).text_muted)
}

fn render_unknown(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> AnyElement {
    label(node.kind.clone(), zoom, Theme::of(cx).text_faint)
}

fn label(text: String, zoom: f32, color: Hsla) -> AnyElement {
    text_style(div(), TextStyle::Callout, zoom)
        .text_color(color)
        .child(text)
        .into_any_element()
}

fn read_text(node: &Node) -> String {
    node.text.clone().unwrap_or_default()
}

fn write_text(node: &mut Node, text: String) {
    node.text = Some(text);
}

fn read_label(node: &Node) -> String {
    node.label.clone().unwrap_or_default()
}

fn write_label(node: &mut Node, label: String) {
    node.label = Some(label);
}
