//! The syntax screen — every grammar `syntax` highlights.
//!
//! Nothing here is library code. The page picks a sample and hands it to
//! [`markdown::render`] as a one-fence document. Copy this file.
//!
//! Samples are chosen so each reaches token kinds the others do not — a wrong
//! palette color has to show up on some screen, and this is it.

use gpui::{Context, ElementId, Render, ScrollHandle, Window, div, prelude::*, px};
use markdown::Doc;
use theme::Theme;
use ui::{
    scroll::{self, TransientState},
    widgets::Controls,
};

use crate::{hint, stack};

/// A fence tag, its rail label, and a sample that reaches kinds the others do
/// not — lifetimes and escapes in Rust, a docstring in Python, booleans in
/// TypeScript, JSX in TSX, keys in JSON, builtins in Go, expansions in Bash,
/// and TOML's own key/value split.
const SAMPLES: &[(&str, &str, &str)] = &[
    (
        "rust",
        "Rust",
        r#"#[derive(Debug)]
struct Orb<'a> {
    name: &'a str,
    size: f32,
}

impl<'a> Orb<'a> {
    // Twelve states, one clock.
    fn label(&self) -> &'a str {
        self.name
    }
}

fn main() {
    let orb = Orb { name: "searching", size: 32.0 };
    println!("{}\n", orb.label());
}"#,
    ),
    (
        "python",
        "Python",
        r#"from math import tau

def spin(t: float, n: int = 12) -> list[float]:
    """Dot angles at time t."""
    return [tau * (i / n + t) for i in range(n)]"#,
    ),
    (
        "typescript",
        "TypeScript",
        r#"type State = "working" | "searching";

// Every state but idle keeps the orb spinning.
export function isBusy(state: State): boolean {
  if (state === "working") return true;
  return false;
}"#,
    ),
    (
        "tsx",
        "TSX",
        r#"const Orb = ({ state }: { state: string }) => (
  <span className="orb" data-state={state}>
    {state}
  </span>
);"#,
    ),
    (
        "json",
        "JSON",
        r#"{
  "name": "bezel",
  "version": "0.0.2",
  "wasm": true,
  "crates": 8
}"#,
    ),
    (
        "go",
        "Go",
        r#"package orb

import "fmt"

type Orb struct {
	Name string
	Size float64
}

// Twelve states, one clock.
func (o Orb) Label() string {
	return fmt.Sprintf("%s\n", o.Name)
}"#,
    ),
    (
        "bash",
        "Bash",
        r#"#!/usr/bin/env bash
set -euo pipefail

# Twelve states, one clock.
for state in working searching solving; do
  printf '%s\n' "${state}" | tr '[:lower:]' '[:upper:]'
done"#,
    ),
    (
        "toml",
        "TOML",
        r#"[package]
name = "bezel"
version = "0.0.2"
edition = "2024"

[dependencies]
gpui = { workspace = true, features = ["wayland"] }

[release]
published = 2026-08-19
wasm = true"#,
    ),
];

pub struct Syntax {
    /// One parsed document per sample, built once — parsing on every render
    /// would put a markdown parse in the scroll path for no gain.
    docs: Vec<Doc>,
    selected: usize,
    scroll: ScrollHandle,
    bar: TransientState,
}

impl Syntax {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            docs: SAMPLES
                .iter()
                .map(|(tag, _, code)| markdown::parse(&format!("```{tag}\n{code}\n```")))
                .collect(),
            selected: 0,
            scroll: ScrollHandle::new(),
            bar: TransientState::new(),
        }
    }
}

impl Render for Syntax {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let doc = self.docs[self.selected].clone();

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("syntax-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "Tree-sitter highlighting for fenced code blocks. A fence tag \
                 names the grammar, `syntax::highlight` returns spans, and \
                 `markdown::render` recolors runs without moving layout — the \
                 block is laid out line by line, so highlighting never shifts \
                 it.",
                            ))
                            .child(
                                theme
                                    .toggle_group()
                                    .children(SAMPLES.iter().enumerate().map(
                                        |(index, (_, label, _))| {
                                            theme
                                                .toggle_group_item(*label, self.selected == index)
                                                .id(ElementId::Name((*label).into()))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.selected = index;
                                                    cx.notify();
                                                }))
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .max_w(px(680.0))
                                    .child(markdown::render(&doc, window, cx)),
                            ),
                    ),
            )
            .child(scroll::transient(
                "syntax-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
