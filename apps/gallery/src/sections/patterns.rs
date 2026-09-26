use crate::*;

impl Gallery {
    pub(crate) fn patterns(
        &mut self,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let section = stack();

        Some(match key {
            // ---- Patterns ----------------------------------------------------
            "agent-activity" => self.activity.clone().into_any_element(),
            "agent-tools" => self.tool_calls.clone().into_any_element(),
            "agent-composer" => self.agent_composer.clone().into_any_element(),
            "agent-transcript" => self.transcript.clone().into_any_element(),
            "agent-diff" => self.diff.clone().into_any_element(),
            "document" => self.document.clone().into_any_element(),
            "selectable-text" => section
                .child(hint(
                    &theme,
                    "Press in the prose and drag. The selection is painted by \
                     the renderer the editor uses and resolves against the same \
                     layouts — what the library adds is the gesture, and \
                     nothing else. Which document holds the selection, and what \
                     copying means, stay the screen's.",
                ))
                .child(self.selectable.clone())
                .into_any_element(),
            "editor" => self.editor.clone().into_any_element(),
            "canvas" => self.canvas.clone().into_any_element(),
            "browser" => self.browser.clone().into_any_element(),
            "ribbon" => self.ribbon.clone().into_any_element(),
            #[cfg(not(target_family = "wasm"))]
            "agent-terminal" => self.terminal.clone().into_any_element(),
            "agent-orbs" => self.orbs.clone().into_any_element(),
            "markdown" => self.dialect.clone().into_any_element(),
            "syntax" => self.syntax.clone().into_any_element(),
            "agent-avatar" => self.avatar.clone().into_any_element(),

            _ => return None,
        })
    }
}
