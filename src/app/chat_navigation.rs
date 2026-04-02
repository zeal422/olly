use ratatui::layout::Alignment;

use super::App;

impl App {
    pub(crate) fn scroll_up(&mut self, amount: u16) {
        let max_scroll = self.chat_max_scroll_hint.get();
        self.chat_scroll = self
            .chat_scroll
            .min(max_scroll)
            .saturating_add(amount)
            .min(max_scroll);
    }

    pub(crate) fn scroll_down(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .min(self.chat_max_scroll_hint.get())
            .saturating_sub(amount);
    }

    pub(crate) fn scroll_up_preserve_selection(&mut self, amount: u16) {
        let max_scroll = self.chat_max_scroll_hint.get();
        self.chat_scroll = self
            .chat_scroll
            .min(max_scroll)
            .saturating_add(amount)
            .min(max_scroll);
    }

    pub(crate) fn scroll_down_preserve_selection(&mut self, amount: u16) {
        self.chat_scroll = self
            .chat_scroll
            .min(self.chat_max_scroll_hint.get())
            .saturating_sub(amount);
    }

    pub(crate) fn update_chat_max_scroll_hint(&self, max_scroll: u16) {
        self.chat_max_scroll_hint.set(max_scroll);
    }

    pub(crate) fn clamp_chat_scroll_to_hint(&mut self) {
        self.chat_scroll = self.chat_scroll.min(self.chat_max_scroll_hint.get());
    }

    pub(crate) fn clear_chat_selection(&mut self) {
        self.chat_selection_anchor = None;
        self.chat_selection_current = None;
        self.is_selecting = false;
        self.auto_scroll_direction = None;
    }

    pub(crate) fn update_chat_area_metadata(
        &mut self,
        area: ratatui::layout::Rect,
        visual_lines: Vec<(String, Option<Alignment>)>,
        max_scroll: u16,
    ) {
        self.chat_area = Some(area);
        self.chat_visual_lines = visual_lines;
        self.update_chat_max_scroll_hint(max_scroll);
    }
}
