use super::{App, ThemePickerState, ToastLevel, UiTheme};

impl App {
    pub fn theme_picker_entries(&self) -> &'static [UiTheme] {
        &UiTheme::ALL
    }

    pub(crate) fn open_theme_picker(&mut self) {
        let selected = UiTheme::ALL
            .iter()
            .position(|theme| *theme == self.theme)
            .unwrap_or(0);
        self.theme_picker = Some(ThemePickerState {
            selected,
            original: self.theme,
        });
        self.set_toast(ToastLevel::Info, "Theme picker opened (live preview)");
    }

    pub(crate) fn theme_picker_next(&mut self) {
        if let Some(state) = &mut self.theme_picker {
            state.selected = (state.selected + 1) % UiTheme::ALL.len();
            self.theme = UiTheme::ALL[state.selected];
            self.chat_content_version += 1;
        }
    }

    pub(crate) fn theme_picker_prev(&mut self) {
        if let Some(state) = &mut self.theme_picker {
            if state.selected == 0 {
                state.selected = UiTheme::ALL.len() - 1;
            } else {
                state.selected -= 1;
            }
            self.theme = UiTheme::ALL[state.selected];
            self.chat_content_version += 1;
        }
    }

    pub(crate) fn theme_picker_confirm(&mut self) {
        if self.theme_picker.take().is_some() {
            self.set_toast(
                ToastLevel::Success,
                format!("Theme set: {}", self.theme.label()),
            );
        }
    }

    pub(crate) fn theme_picker_cancel(&mut self) {
        if let Some(state) = self.theme_picker.take() {
            self.theme = state.original;
            self.chat_content_version += 1;
            self.set_toast(ToastLevel::Info, "Theme picker canceled");
        }
    }
}
