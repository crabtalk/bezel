//! Every action the canvas answers to, and the chords bound to it.

use gpui::{KeyBinding, actions};

actions!(
    bezel_canvas,
    [
        AddChild,
        AddSibling,
        Remove,
        Edit,
        StopEditing,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        NudgeLeft,
        NudgeRight,
        NudgeUp,
        NudgeDown,
        SelectAll,
        Deselect,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        Duplicate,
        Fit,
        ZoomToSelection,
        ZoomIn,
        ZoomOut,
        ResetZoom,
    ]
);

/// The default keymap, as data. Escape is bound over the editor's own so
/// it leaves a node, which is why [`super::init`] runs after `editor::init`.
pub fn bindings() -> Vec<KeyBinding> {
    let ctx = Some(super::CONTEXT);
    let editing = format!("{} > {}", super::CONTEXT, editor::CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("tab", AddChild, ctx),
        KeyBinding::new("enter", AddSibling, ctx),
        KeyBinding::new("backspace", Remove, ctx),
        KeyBinding::new("delete", Remove, ctx),
        KeyBinding::new("f2", Edit, ctx),
        KeyBinding::new("left", SelectLeft, ctx),
        KeyBinding::new("right", SelectRight, ctx),
        KeyBinding::new("up", SelectUp, ctx),
        KeyBinding::new("down", SelectDown, ctx),
        KeyBinding::new("shift-left", NudgeLeft, ctx),
        KeyBinding::new("shift-right", NudgeRight, ctx),
        KeyBinding::new("shift-up", NudgeUp, ctx),
        KeyBinding::new("shift-down", NudgeDown, ctx),
        KeyBinding::new("escape", Deselect, ctx),
        KeyBinding::new("shift-1", Fit, ctx),
        KeyBinding::new("shift-2", ZoomToSelection, ctx),
        KeyBinding::new("escape", StopEditing, Some(&editing)),
    ];
    let platform = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let chord = |key: &str| format!("{platform}-{key}");
    bindings.extend([
        KeyBinding::new(&chord("a"), SelectAll, ctx),
        KeyBinding::new(&chord("z"), Undo, ctx),
        KeyBinding::new(&chord("shift-z"), Redo, ctx),
        KeyBinding::new(&chord("c"), Copy, ctx),
        KeyBinding::new(&chord("x"), Cut, ctx),
        KeyBinding::new(&chord("v"), Paste, ctx),
        KeyBinding::new(&chord("d"), Duplicate, ctx),
        KeyBinding::new(&chord("="), ZoomIn, ctx),
        KeyBinding::new(&chord("-"), ZoomOut, ctx),
        KeyBinding::new(&chord("0"), ResetZoom, ctx),
    ]);
    bindings
}
