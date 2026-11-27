use super::Settings;

impl Settings {
    /// Puts on the window whatever whoever uses it configured, **except the font**.
    ///
    /// The family named in the file is applied by [`crate::fonts::choose_the_font`] instead, and it is not tidiness:
    /// everything here puts a value on the window, while that one talks to the text engine, needs the
    /// context and can cost tens of milliseconds. It runs beside this, at start-up.
    ///
    /// **It goes here and not in the constructor**, just like the injected drags and the restoring
    /// span: taking it inside would force passing it through three constructors with dozens of
    /// calls, and none of them configures anything.
    pub(crate) fn apply_to(&self, window: &mut crate::Window) {
        crate::board::configure(window, self);
        window.chrome.agent_preferences = crate::agent_signals::Enabled {
            claude: self.agent_claude,
            codex: self.agent_codex,
            opencode: self.agent_opencode,
        };
        window.theme = self.resolved_theme();
        // A complete document replaces the preceding configuration. Omitting an override must
        // restore the same defaults as a fresh startup, including shortcuts previously unbound.
        window.font_size = self.font_size.unwrap_or(crate::DEFAULT_FONT_SIZE);
        window.chrome.title_sizes = crate::chrome::typography::Sizes::new(
            self.pane_title_size.unwrap_or(12.0),
            self.tab_label_size.unwrap_or(12.0),
        );
        window.copy_enabled = self.copy.unwrap_or(true);
        window.ligatures = self.ligatures.unwrap_or(false);
        window.gentle_enabled = self.gentle_enabled;
        window.blink.ceiling = self.blink;
        window.keymap = crate::keyboard::Keymap::default();
        // **They are applied in the order they were written, and that is why they arrive as a
        // list.** «The last one wins» can only be decided that way, and it is what makes pasting
        // one set of shortcuts on top of another do what is expected.
        for (action, chord) in &self.shortcuts {
            let displaced = chord
                .map(|chord| window.keymap.displaced_by(*action, chord))
                .unwrap_or_default();
            window.keymap.bind(*action, *chord);
            for displaced in displaced {
                // **The clash is said out loud, not resolved in silence.** Without this, whoever
                // reassigns a chord that already had an owner loses the other shortcut without
                // noticing, and finds out the day they need it.
                eprintln!(
                    "ivy: '{}' is left without a shortcut: its chord goes to '{}', which comes later",
                    displaced.as_written(),
                    action.as_written()
                );
            }
        }
    }
}
