use crate::renderer::backends::html_interactive::style::CssStyle;

impl CssStyle {
    pub(crate) fn apply_display(&mut self, value: &str) {
        let normalized = value.trim().to_ascii_lowercase();
        if self.apply_inline_display(&normalized) {
            return;
        }
        let Ok(display) = value.parse() else {
            return;
        };
        self.display = display;
        self.inline_block = false;
        self.inline_atomic = false;
    }

    pub(crate) fn apply_inline_display(&mut self, value: &str) -> bool {
        let Some((display, atomic)) = inline_display(value) else {
            return false;
        };
        self.display = display;
        self.inline_block = true;
        self.inline_atomic = atomic;
        true
    }

    pub(crate) fn apply_color(&mut self, value: &str) {
        self.color = value.to_string();
        self.explicit_color = true;
    }

    pub(crate) fn apply_background(&mut self, value: &str) {
        self.background = Some(value.to_string());
        self.explicit_background = true;
    }
}

fn inline_display(value: &str) -> Option<(taffy::style::Display, bool)> {
    match value {
        "inline" => Some((taffy::style::Display::Block, false)),
        "inline-block" => Some((taffy::style::Display::Block, true)),
        "inline-flex" => Some((taffy::style::Display::Flex, true)),
        "inline-grid" => Some((taffy::style::Display::Grid, true)),
        _ => None,
    }
}
