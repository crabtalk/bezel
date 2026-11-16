//! Every action the canvas answers to, and the chords bound to it.

use crate::view::{self, editing};
use gpui::{App, KeyBinding, actions};

actions!(
    bezel_canvas,
    [
        AddChild,
        AddSibling,
        Remove,
        Edit,
        StopEditing,
        ConfirmEditing,
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
pub fn bindings(cx: &App) -> Vec<KeyBinding> {
    let ctx = Some(view::CONTEXT);
    let editing = format!("{} > {}", view::CONTEXT, editor::CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("enter", ConfirmEditing, Some(editing::FIELD_CONTEXT)),
        KeyBinding::new("escape", StopEditing, Some(editing::FIELD_CONTEXT)),
        KeyBinding::new("escape", StopEditing, Some(editing::TEXT_CONTEXT)),
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
    let is_mac = cx.platform_info().is_mac();
    bindings.extend([
        KeyBinding::new(if is_mac { "cmd-a" } else { "ctrl-a" }, SelectAll, ctx),
        KeyBinding::new(if is_mac { "cmd-z" } else { "ctrl-z" }, Undo, ctx),
        KeyBinding::new(
            if is_mac {
                "cmd-shift-z"
            } else {
                "ctrl-shift-z"
            },
            Redo,
            ctx,
        ),
        KeyBinding::new(if is_mac { "cmd-c" } else { "ctrl-c" }, Copy, ctx),
        KeyBinding::new(if is_mac { "cmd-x" } else { "ctrl-x" }, Cut, ctx),
        KeyBinding::new(if is_mac { "cmd-v" } else { "ctrl-v" }, Paste, ctx),
        KeyBinding::new(if is_mac { "cmd-d" } else { "ctrl-d" }, Duplicate, ctx),
        KeyBinding::new(if is_mac { "cmd-=" } else { "ctrl-=" }, ZoomIn, ctx),
        KeyBinding::new(if is_mac { "cmd--" } else { "ctrl--" }, ZoomOut, ctx),
        KeyBinding::new(if is_mac { "cmd-0" } else { "ctrl-0" }, ResetZoom, ctx),
    ]);
    bindings
}
