use super::page::ChromiumPage;
use crate::{HtmlBrowserInput, HtmlBrowserNavigationEvent};
use headless_chrome::{browser::tab::point::Point, protocol::cdp::Input};
use serde_json::Value;

impl ChromiumPage {
    pub(super) fn input(&mut self, input: HtmlBrowserInput) -> Result<(), String> {
        match input {
            HtmlBrowserInput::Focus { focused } => self.focus(focused)?,
            HtmlBrowserInput::PointerMove { x, y } => self.move_mouse(x, y)?,
            HtmlBrowserInput::PointerDown { x, y, button } => {
                self.pointer_down = Some((x, y, button));
                self.move_mouse(x, y)?;
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
        self.synchronize_rendering()?;
        Ok(())
    }

    pub(super) fn take_navigation(&self) -> Result<Option<HtmlBrowserNavigationEvent>, String> {
        let value = self
            .tab
            .evaluate("window.__katanaNavigation || null", false)
            .map_err(string_error)?
            .value;
        navigation_from_value(value)
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
        self.tab
            .click_point(Point {
                x: f64::from(x),
                y: f64::from(y),
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

    fn focus(&self, focused: bool) -> Result<(), String> {
        let script = if focused {
            "window.focus(); const target = document.activeElement === document.body ? document.querySelector('[autofocus]') : null; if (target && target.focus) target.focus(); true;"
        } else {
            "if (document.activeElement && document.activeElement.blur) document.activeElement.blur(); window.blur(); true;"
        };
        self.tab
            .activate()
            .and_then(|tab| tab.evaluate(script, false))
            .map(|_| ())
            .map_err(string_error)
    }

    pub(super) fn synchronize_rendering(&self) -> Result<(), String> {
        self.tab
            .evaluate(
                "Promise.resolve().then(() => { document.documentElement.getBoundingClientRect(); return new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); })",
                true,
            )
            .map(|_| ())
            .map_err(string_error)
    }
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

fn navigation_from_value(
    value: Option<Value>,
) -> Result<Option<HtmlBrowserNavigationEvent>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let url = serde_json::from_value::<Option<String>>(value).map_err(string_error)?;
    url.map(HtmlBrowserNavigationEvent::new)
        .transpose()
        .map_err(|error| error.to_string())
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
    fn navigation_from_value_accepts_missing_null_and_valid_urls() {
        assert!(matches!(navigation_from_value(None), Ok(None)));
        assert!(matches!(
            navigation_from_value(Some(serde_json::Value::Null)),
            Ok(None)
        ));
        assert!(matches!(
            navigation_from_value(Some(serde_json::json!("https://example.test/next"))),
            Ok(Some(event)) if event.url.as_str() == "https://example.test/next"
        ));
    }

    #[test]
    fn navigation_from_value_rejects_invalid_values_and_urls() {
        assert!(navigation_from_value(Some(serde_json::json!({ "url": "bad" }))).is_err());
        assert!(navigation_from_value(Some(serde_json::json!("not a url"))).is_err());
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
