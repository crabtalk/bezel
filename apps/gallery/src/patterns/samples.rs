// The samples the Syntax page shows. Plain `//` because `build.rs` includes
// this file to highlight them ahead of time, and an included file may carry no
// module doc — which is also why nothing but the const lives here.

/// A fence tag, its rail label, and a sample that reaches kinds the others do
/// not — lifetimes and escapes in Rust, a docstring in Python, booleans in
/// TypeScript, JSX in TSX, keys in JSON, builtins in Go, expansions in Bash,
/// and TOML's own key/value split.
pub const SAMPLES: &[(&str, &str, &str)] = &[
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
