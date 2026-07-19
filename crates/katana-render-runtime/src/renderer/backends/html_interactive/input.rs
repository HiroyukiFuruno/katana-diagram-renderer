use super::super::html_browser::{HtmlBrowserInput, HtmlBrowserNavigationEvent};
use super::super::html_runtime::{HtmlNodeId, HtmlRuntimeDispatch, HtmlRuntimeEvent};
use super::constants::LEFT_MOUSE_BUTTON;
use super::types::HitTargetKind;
use super::{HtmlBrowserError, HtmlInteractiveSession, runtime_failure};

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn dispatch_input(
        &mut self,
        input: HtmlBrowserInput,
    ) -> Result<(), HtmlBrowserError> {
        input.validate()?;
        match input {
            HtmlBrowserInput::Focus { focused } => self.update_focus(focused),
            HtmlBrowserInput::PointerMove { .. }
            | HtmlBrowserInput::KeyDown { .. }
            | HtmlBrowserInput::KeyUp { .. } => {}
            HtmlBrowserInput::PointerDown { x, y, button } => self.press_target(x, y, button),
            HtmlBrowserInput::PointerUp { x, y, button } => self.release_target(x, y, button)?,
            HtmlBrowserInput::Scroll { delta_y, .. } => self.scroll(delta_y)?,
            HtmlBrowserInput::Text { text } => self.append_text(&text)?,
        }
        Ok(())
    }

    fn update_focus(&mut self, focused: bool) {
        if !focused {
            self.focused_input = None;
        }
    }

    fn press_target(&mut self, x: f32, y: f32, button: u8) {
        self.pressed_target = (button == LEFT_MOUSE_BUTTON)
            .then(|| self.hit_target_at(x, y).map(|target| target.node_id))
            .flatten();
    }

    fn release_target(&mut self, x: f32, y: f32, button: u8) -> Result<(), HtmlBrowserError> {
        if button == LEFT_MOUSE_BUTTON {
            self.activate_target_at(x, y)?;
        }
        self.pressed_target = None;
        Ok(())
    }

    fn scroll(&mut self, delta_y: f32) -> Result<(), HtmlBrowserError> {
        self.scroll_y = (self.scroll_y + delta_y).clamp(0.0, self.max_scroll());
        self.render_frame()
    }

    fn activate_target_at(&mut self, x: f32, y: f32) -> Result<(), HtmlBrowserError> {
        let Some(target) = self.hit_target_at(x, y).cloned() else {
            return Ok(());
        };
        if self.pressed_target != Some(target.node_id) {
            return Ok(());
        }
        match target.kind {
            HitTargetKind::Input => self.focus_input(target.node_id),
            HitTargetKind::Summary { details_node_id } => {
                self.toggle_details(target.node_id, details_node_id)
            }
            HitTargetKind::Click => self.dispatch_click(target.node_id),
        }
    }

    fn focus_input(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        self.focused_input = Some(node_id);
        self.input_values.entry(node_id).or_default();
        self.render_frame()
    }

    fn toggle_details(
        &mut self,
        summary_node_id: u64,
        details_node_id: u64,
    ) -> Result<(), HtmlBrowserError> {
        self.dispatch_click(summary_node_id)?;
        self.runtime
            .toggle_open(details_node_id)
            .map_err(runtime_failure)?;
        self.dispatch_toggle(details_node_id)
    }

    fn append_text(&mut self, text: &str) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        let value = self.input_values.entry(node_id).or_default();
        value.push_str(text);
        self.runtime
            .set_value(node_id, value)
            .map_err(runtime_failure)?;
        self.dispatch_runtime_event(HtmlRuntimeEvent::Input {
            target: HtmlNodeId(node_id),
        })
    }

    fn dispatch_click(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        let dispatch = self
            .runtime
            .dispatch(HtmlRuntimeEvent::Click {
                target: HtmlNodeId(node_id),
            })
            .map_err(runtime_failure)?;
        self.accept_navigation(dispatch)?;
        self.render_frame()
    }

    fn dispatch_toggle(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        self.dispatch_runtime_event(HtmlRuntimeEvent::Toggle {
            target: HtmlNodeId(node_id),
        })
    }

    fn dispatch_runtime_event(&mut self, event: HtmlRuntimeEvent) -> Result<(), HtmlBrowserError> {
        self.runtime.dispatch(event).map_err(runtime_failure)?;
        self.render_frame()
    }

    fn accept_navigation(&mut self, dispatch: HtmlRuntimeDispatch) -> Result<(), HtmlBrowserError> {
        let Some(intent) = dispatch.navigation else {
            return Ok(());
        };
        let url = self
            .resource_policy
            .resolve_navigation(&intent.href)
            .map_err(navigation_error)?;
        self.pending_navigation = Some(HtmlBrowserNavigationEvent { url });
        Ok(())
    }
}

fn navigation_error(error: String) -> HtmlBrowserError {
    if error.starts_with("resource URL is invalid") {
        return runtime_failure(format!("link target is invalid: {error}"));
    }
    runtime_failure(error)
}
