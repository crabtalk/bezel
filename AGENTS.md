# Bezel for agents

Read this before changing anything, and [CONTRIBUTING.md](CONTRIBUTING.md) for
the laws every change follows. `CLAUDE.md` imports this file.

# Role

Bezel is the UI toolkit Cydonia and other apps build on. Its defaults are the
platform's standard design: what AppKit and SwiftUI do on macOS, measured and
dated (CONTRIBUTING.md, laws 1 and 5). A host app's own look is never a bezel
default.

- A host that wants something else overrides it through what bezel exposes:
  modifiers on the returned element, a view's options (`canvas::Options`),
  theme family tokens, `Brand`.
- Change a default only where it differs from the platform's, and record what
  was measured, where and when.
- When a host needs a setting bezel lacks, add the option, with the platform's
  value as its default.

## Overrides

Where bezel's default is deliberately not the platform's. Each is a setting a
host can turn back.

- Selection: a selected row or tab on window chrome is regular Liquid Glass in
  both appearances (`SelectionMaterial::Glass`). `SelectionMaterial::Fill` is
  the flat fill.
