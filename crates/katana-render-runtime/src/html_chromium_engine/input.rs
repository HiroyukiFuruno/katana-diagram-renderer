use super::{page::ChromiumPage, trace};
use crate::{HtmlBrowserInput, HtmlBrowserNavigationEvent};
use headless_chrome::{
    browser::tab::point::Point,
    protocol::cdp::{Emulation, Input},
};

impl ChromiumPage {
    pub(super) fn input(&mut self, input: HtmlBrowserInput) -> Result<(), String> {
        trace::stage("page:input:start");
        match input {
            HtmlBrowserInput::Focus { focused } => self.focus(focused)?,
            HtmlBrowserInput::PointerMove { x, y } => self.move_mouse(x, y)?,
            HtmlBrowserInput::PointerDown { x, y, button } => {
                self.pointer_down = Some((x, y, button));
                self.tab.activate().map_err(string_error)?;
                self.move_mouse(x, y)?;
                if button == 0 {
                    self.primary_pointer_down(x, y)?;
                }
            }
            HtmlBrowserInput::PointerUp { x, y, button } => self.pointer_up(x, y, button)?,
            HtmlBrowserInput::Text { text } => {
                self.tab.type_str(&text).map_err(string_error)?;
            }
            HtmlBrowserInput::KeyDown { key } => {
                self.tab.press_key(&key).map_err(string_error)?;
            }
            HtmlBrowserInput::KeyUp { .. } => {}
            HtmlBrowserInput::Scroll { delta_x, delta_y } => self.scroll(delta_x, delta_y)?,
        }
        if self.focused {
            trace::stage("page:input:synchronize");
            self.synchronize_rendering()?;
        }
        trace::stage("page:input:ready");
        Ok(())
    }

    pub(super) fn take_navigation(&mut self) -> Result<Option<HtmlBrowserNavigationEvent>, String> {
        let wait_for_popup = self.navigation.has_pending_popup();
        let closed = self
            .popup_guard
            .close_new_targets(wait_for_popup)
            .map_err(string_error)?;
        self.navigation.confirm_closed_popups(closed);
        self.navigation.take()
    }

    fn move_mouse(&self, x: f32, y: f32) -> Result<(), String> {
        self.tab
            .move_mouse_to_point(Point {
                x: f64::from(x),
                y: f64::from(y),
            })
            .map(|_| ())
            .map_err(string_error)
    }

    fn pointer_up(&mut self, x: f32, y: f32, button: u8) -> Result<(), String> {
        let clicked = is_primary_click(self.pointer_down.take(), button);
        if !clicked {
            return Ok(());
        }
        self.primary_pointer_up(x, y)
    }

    fn primary_pointer_down(&self, x: f32, y: f32) -> Result<(), String> {
        self.tab
            .call_method(Input::DispatchMouseEvent {
                Type: Input::DispatchMouseEventTypeOption::MousePressed,
                x: f64::from(x),
                y: f64::from(y),
                modifiers: None,
                timestamp: None,
                button: Some(Input::MouseButton::Left),
                buttons: Some(1),
                click_count: Some(1),
                force: None,
                tangential_pressure: None,
                tilt_x: None,
                tilt_y: None,
                twist: None,
                delta_x: None,
                delta_y: None,
                pointer_Type: None,
            })
            .map(|_| ())
            .map_err(string_error)
    }

    fn primary_pointer_up(&self, x: f32, y: f32) -> Result<(), String> {
        self.tab
            .call_method(Input::DispatchMouseEvent {
                Type: Input::DispatchMouseEventTypeOption::MouseReleased,
                x: f64::from(x),
                y: f64::from(y),
                modifiers: None,
                timestamp: None,
                button: Some(Input::MouseButton::Left),
                buttons: Some(0),
                click_count: Some(1),
                force: None,
                tangential_pressure: None,
                tilt_x: None,
                tilt_y: None,
                twist: None,
                delta_x: None,
                delta_y: None,
                pointer_Type: None,
            })
            .map(|_| ())
            .map_err(string_error)
    }

    fn scroll(&self, delta_x: f32, delta_y: f32) -> Result<(), String> {
        self.tab
            .call_method(Input::DispatchMouseEvent {
                Type: Input::DispatchMouseEventTypeOption::MouseWheel,
                x: 0.0,
                y: 0.0,
                modifiers: None,
                timestamp: None,
                button: None,
                buttons: None,
                click_count: None,
                force: None,
                tangential_pressure: None,
                tilt_x: None,
                tilt_y: None,
                twist: None,
                delta_x: Some(f64::from(delta_x)),
                delta_y: Some(f64::from(delta_y)),
                pointer_Type: None,
            })
            .map(|_| ())
            .map_err(string_error)
    }

    fn focus(&mut self, focused: bool) -> Result<(), String> {
        let script = if focused {
            "window.focus(); const target = document.activeElement === document.body ? document.querySelector('[autofocus]') : null; if (target && target.focus) target.focus(); true;"
        } else {
            "if (document.activeElement && document.activeElement.blur) document.activeElement.blur(); window.blur(); true;"
        };
        self.tab.activate().map_err(string_error)?;
        self.emulate_focus(focused)?;
        self.tab.evaluate(script, false).map_err(string_error)?;
        self.focused = focused;
        Ok(())
    }

    pub(super) fn emulate_focus(&self, focused: bool) -> Result<(), String> {
        self.tab
            .call_method(Emulation::SetFocusEmulationEnabled { enabled: focused })
            .map(|_| ())
            .map_err(string_error)
    }
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

fn is_primary_click(pointer_down: Option<(f32, f32, u8)>, button: u8) -> bool {
    let Some((_, _, down_button)) = pointer_down else {
        return false;
    };
    if down_button != button {
        return false;
    }
    button == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_error_preserves_display_message() {
        assert_eq!(string_error("input failed"), "input failed");
    }

    #[test]
    fn primary_click_requires_matching_left_button_down_and_up() {
        assert!(is_primary_click(Some((1.0, 2.0, 0)), 0));
        assert!(!is_primary_click(Some((1.0, 2.0, 1)), 1));
        assert!(!is_primary_click(Some((1.0, 2.0, 0)), 1));
        assert!(!is_primary_click(None, 0));
    }
}
#[test]
fn string_error_preserves_display_message() {
    assert_eq!(string_error("input failed"), "input failed");
}
