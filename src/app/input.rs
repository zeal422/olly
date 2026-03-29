use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::KeybindingsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    Send,
    Insert(char),
    NewLine,
    Backspace,
    ScrollUp(u16),
    ScrollDown(u16),
    MoveCursorLeft,
    MoveCursorRight,
    ToggleHelp,
    DismissOverlay,
    SwitchFocus,
    AutoCompleteCommand,
    Quit,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone)]
pub struct InputBindings {
    pub send: InputBinding,
    pub newline: InputBinding,
    pub quit: InputBinding,
    pub help: InputBinding,
    pub dismiss: InputBinding,
    pub switch_focus: InputBinding,
    pub autocomplete: InputBinding,
}

impl Default for InputBinding {
    fn default() -> Self {
        Self {
            code: KeyCode::Null,
            modifiers: KeyModifiers::NONE,
        }
    }
}

impl InputBinding {
    pub fn from_string(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut modifiers = KeyModifiers::NONE;
        let mut key_token = None;
        for part in trimmed.split('+') {
            let token = part.trim().to_lowercase();
            match token.as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                other => key_token = Some(other.to_string()),
            }
        }
        let token = key_token?;

        let code = match token.as_str() {
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "esc" | "escape" => KeyCode::Esc,
            "f1" => KeyCode::F(1),
            "f2" => KeyCode::F(2),
            "f3" => KeyCode::F(3),
            "f4" => KeyCode::F(4),
            "f5" => KeyCode::F(5),
            "f6" => KeyCode::F(6),
            "f7" => KeyCode::F(7),
            "f8" => KeyCode::F(8),
            "f9" => KeyCode::F(9),
            "f10" => KeyCode::F(10),
            "f11" => KeyCode::F(11),
            "f12" => KeyCode::F(12),
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "pgup" | "pageup" => KeyCode::PageUp,
            "pgdn" | "pagedown" => KeyCode::PageDown,
            "backspace" => KeyCode::Backspace,
            t if t.len() == 1 => KeyCode::Char(t.chars().next()?),
            _ => return None,
        };

        Some(Self { code, modifiers })
    }

    fn matches(&self, event: &KeyEvent) -> bool {
        event.code == self.code && event.modifiers.contains(self.modifiers)
    }
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            send: InputBinding::from_string("Ctrl+Enter").unwrap_or_default(),
            newline: InputBinding::from_string("Enter").unwrap_or_default(),
            quit: InputBinding::from_string("Ctrl+Q").unwrap_or_default(),
            help: InputBinding::from_string("F1").unwrap_or_default(),
            dismiss: InputBinding::from_string("Esc").unwrap_or_default(),
            switch_focus: InputBinding::from_string("Tab").unwrap_or_default(),
            autocomplete: InputBinding::from_string("Tab").unwrap_or_default(),
        }
    }
}

impl InputBindings {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let defaults = Self::default();
        let configured_help = InputBinding::from_string(&config.help);
        let normalized_help = match configured_help {
            // Keep '?' available for normal typing in chat.
            Some(InputBinding {
                code: KeyCode::Char('?'),
                modifiers,
            }) if modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT => {
                defaults.help
            }
            Some(binding) => binding,
            None => defaults.help,
        };
        Self {
            send: InputBinding::from_string(&config.send).unwrap_or(defaults.send),
            newline: InputBinding::from_string(&config.newline).unwrap_or(defaults.newline),
            quit: InputBinding::from_string(&config.quit).unwrap_or(defaults.quit),
            help: normalized_help,
            dismiss: InputBinding::from_string(&config.dismiss).unwrap_or(defaults.dismiss),
            switch_focus: InputBinding::from_string(&config.switch_focus)
                .unwrap_or(defaults.switch_focus),
            autocomplete: InputBinding::from_string(&config.autocomplete)
                .unwrap_or(defaults.autocomplete),
        }
    }
}

pub fn resolve_key_event(event: KeyEvent) -> InputAction {
    resolve_key_event_with_bindings(event, &InputBindings::default(), "")
}

pub fn resolve_key_event_with_bindings(
    event: KeyEvent,
    bindings: &InputBindings,
    current_input: &str,
) -> InputAction {
    if event.kind != KeyEventKind::Press {
        return InputAction::Noop;
    }

    if bindings.quit.matches(&event) {
        return InputAction::Quit;
    }
    if bindings.help.matches(&event) {
        return InputAction::ToggleHelp;
    }
    if bindings.dismiss.matches(&event) {
        return InputAction::DismissOverlay;
    }
    if bindings.send.matches(&event) {
        return InputAction::Send;
    }
    if bindings.newline.matches(&event) {
        return InputAction::NewLine;
    }
    if bindings.autocomplete.matches(&event)
        && current_input.trim_start().starts_with('/')
        && !current_input.contains('\n')
    {
        return InputAction::AutoCompleteCommand;
    }
    if bindings.switch_focus.matches(&event) {
        return InputAction::SwitchFocus;
    }

    match event.code {
        KeyCode::Up => InputAction::ScrollUp(1),
        KeyCode::Down => InputAction::ScrollDown(1),
        KeyCode::PageUp => InputAction::ScrollUp(8),
        KeyCode::PageDown => InputAction::ScrollDown(8),
        KeyCode::Left => InputAction::MoveCursorLeft,
        KeyCode::Right => InputAction::MoveCursorRight,
        KeyCode::Backspace => InputAction::Backspace,
        KeyCode::Char(c) => InputAction::Insert(c),
        _ => InputAction::Noop,
    }
}
