---
title: Ribbon
description: A formatting bar that is always on, painted from one call — the marks that are lit, the block's name, what cmd-E would do, and whether any of it applies.
---

Nothing on this page is library code. It is a docked bar over an `Editor`, and the whole of it is one call:

```rust
let formatting = editor.read(cx).formatting();

let lit = formatting.marks.contains(&mark);          // and cmd-B before typing counts
let label = formatting.block.clone();                // "Heading 2", for the dropdown
let fence = formatting.fenceable;                    // cmd-E would fence, not span
let live = formatting.mode == editor::Mode::Blocks;  // the source has nothing to light
```

**One read, not four.** Each of those was a separate reach into the document before, and a bar that answered half its questions from this frame and half from the last lights the wrong button for a frame. Taken together, it cannot.

**`marks` is what a button means, not what the text looks like.** Over a selection it is what the whole selection carries — drag across a bold word and a plain one and the button is unlit, because pressing it will bold the rest. At a collapsed caret it is what the *next character typed* would carry, which is the left-sticky rule the model already follows, so the button stays lit between `cmd-B` and the letter that spends it.

**`fenceable` is the one thing a bar cannot work out for itself.** `Mark::Code` over more than one line makes a fence instead of an inline span; over a table it does neither. Counting newlines in the selection gets that wrong, so the editor answers instead.

**The dropdown is the library's own vocabulary.** `editor::turns()` is the list the slash menu and the gutter handle offer — label and `BlockKind` per row — and `formatting.block` is the label of the caret's block, matched on kind alone so a numbered list at 7 and a fence tagged `rs` still report their row. A menu you build cannot drift from the two bezel opens.

```rust
for (label, kind) in editor::turns() {
    let lit = formatting.block.as_ref() == Some(&label);
    // on click: editor.update(cx, |editor, cx| editor.set_block(block, kind.clone(), cx));
}
```

**Two of the six buttons are the app's own marks.** `markdown` has no underline and no highlight — CommonMark spells neither — so this gallery registers them itself, with the delimiters that write them and the paint that shows them:

```rust
markdown::set_marks(cx, markdown::Marks::new().with("highlight", "==").with("underline", "++"));
markdown::set_mark_paint(cx, paint);
```

`toggle_mark(Mark::Custom("highlight".into()))` is the same call bold takes, `formatting().marks` lights it the same way, and the editor never learns what the name means. See [Document](/docs/document) for what a registered mark can and cannot reach.

**Disabled is reported, not guessed.** Switch the bar to Markdown and everything left of the toggle greys out: [source mode](/docs/editor) is the markup spelled out, `toggle_mark` refuses there, and a bar that stayed lit would promise something the editor will not do.

The bar is **docked**, so it reflows the document under it — which is why it is a plain row with a hairline rather than [`control_bar`](/docs/control-bar), the floating kind that must never move what it sits over.

The other answer to the same question is the bubble toolbar on the [Editor](/docs/editor) page, which appears at the selection and reads the same snapshot. An app picks one; shipping both is two places to keep in step.

The source is at `apps/gallery/src/patterns/ribbon.rs`. Copy the file.
