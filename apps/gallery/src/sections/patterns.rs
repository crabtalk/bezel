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
            "agent-activity" => self.patterns.activity.clone().into_any_element(),
            "agent-tools" => self.patterns.tool_calls.clone().into_any_element(),
            "agent-composer" => self.patterns.agent_composer.clone().into_any_element(),
            "agent-transcript" => self.patterns.transcript.clone().into_any_element(),
            "agent-diff" => self.patterns.diff.clone().into_any_element(),
            "document" => self.patterns.document.clone().into_any_element(),
            "selectable-text" => section
                .child(hint(
                    &theme,
                    "Press in the prose and drag. The selection is painted by \
                     the renderer the editor uses and resolves against the same \
                     layouts — what the library adds is the gesture, and \
                     nothing else. Which document holds the selection, and what \
                     copying means, stay the screen's.",
                ))
                .child(self.patterns.selectable.clone())
                .into_any_element(),
            "editor" => self.patterns.editor.clone().into_any_element(),
            "canvas" => self.patterns.canvas.clone().into_any_element(),
            "browser" => self.patterns.browser.clone().into_any_element(),
            "ribbon" => self.patterns.ribbon.clone().into_any_element(),
            #[cfg(not(target_family = "wasm"))]
            "agent-terminal" => self.patterns.terminal.clone().into_any_element(),
            "agent-orbs" => self.patterns.orbs.clone().into_any_element(),
            "markdown" => self.patterns.dialect.clone().into_any_element(),
            "syntax" => self.patterns.syntax.clone().into_any_element(),
            "agent-avatar" => self.patterns.avatar.clone().into_any_element(),

            _ => return None,
        })
    }
}
