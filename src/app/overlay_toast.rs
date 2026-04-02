use std::time::Instant;

use super::{App, ToastLevel};

impl App {
    pub(crate) fn set_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(super::Toast {
            message: message.into(),
            level,
            expires_at: Instant::now() + std::time::Duration::from_secs(4),
        });
    }

    pub(crate) fn dismiss_overlay(&mut self) {
        if self.show_help {
            self.show_help = false;
            return;
        }
        self.toast = None;
    }

    pub(crate) fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub(crate) fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if let Some(toast) = &self.toast {
            if Instant::now() > toast.expires_at {
                self.toast = None;
            }
        }
        if let Some(pending_delete) = &self.pending_workspace_delete {
            if Instant::now() > pending_delete.expires_at {
                self.pending_workspace_delete = None;
            }
        }
        if let Some(pending_delete) = &self.pending_session_delete {
            if Instant::now() > pending_delete.expires_at {
                self.pending_session_delete = None;
            }
        }
        if let Some(pending_tool) = &self.pending_tool_confirmation {
            if Instant::now() > pending_tool.expires_at {
                self.pending_tool_confirmation = None;
            }
        }

        // Apply auto-scroll if active during selection drag
        if let Some(direction) = self.auto_scroll_direction {
            if direction < 0 {
                self.scroll_up_preserve_selection(self.auto_scroll_speed);
            } else {
                self.scroll_down_preserve_selection(self.auto_scroll_speed);
            }
        }
    }
}
