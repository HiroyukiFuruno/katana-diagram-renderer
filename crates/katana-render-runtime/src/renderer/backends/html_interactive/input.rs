use super::super::html_browser::{HtmlBrowserInput, HtmlBrowserNavigationEvent};
use super::super::html_runtime::{HtmlNodeId, HtmlRuntimeDispatch, HtmlRuntimeEvent};
use super::constants::LEFT_MOUSE_BUTTON;
use super::types::HitTargetKind;
use super::{HtmlBrowserError, HtmlInteractiveSession, runtime_failure};
use percent_encoding::percent_decode_str;

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn dispatch_input(
        &mut self,
        input: HtmlBrowserInput,
    ) -> Result<(), HtmlBrowserError> {
        input.validate()?;
        match input {
            HtmlBrowserInput::Focus { focused } => self.update_focus(focused)?,
            HtmlBrowserInput::PointerMove { x, y } => self.update_hover(x, y)?,
            HtmlBrowserInput::KeyDown { key } => self.dispatch_key_down(key)?,
            HtmlBrowserInput::KeyUp { key } => self.dispatch_key_up(key)?,
            HtmlBrowserInput::PointerDown { x, y, button } => self.press_target(x, y, button),
            HtmlBrowserInput::PointerUp { x, y, button } => self.release_target(x, y, button)?,
            HtmlBrowserInput::Scroll { delta_y, .. } => self.scroll(delta_y)?,
            HtmlBrowserInput::Text { text } => self.append_text(&text)?,
        }
        Ok(())
    }

    fn update_hover(&mut self, x: f32, y: f32) -> Result<(), HtmlBrowserError> {
        let hovered_node = self.element_at(x, y).map(|element| element.node_id);
        let hovered_nodes = hovered_node.map_or_else(
            || Ok(std::collections::HashSet::new()),
            |node_id| self.runtime.node_path(node_id).map_err(runtime_failure),
        )?;
        if hovered_nodes == self.hovered_nodes {
            return Ok(());
        }
        self.hovered_nodes = hovered_nodes;
        self.render_frame()
    }

    fn press_target(&mut self, x: f32, y: f32, button: u8) {
        let hit = self.hit_target_at(x, y).cloned();
        let target_count = self.hit_targets.len();
        tracing::debug!(
            physical_x = x,
            physical_y = y,
            device_scale_factor = self.viewport.device_scale_factor,
            target_count,
            target = ?hit,
            "HTML pointer down hit-test"
        );
        self.pressed_target = (button == LEFT_MOUSE_BUTTON)
            .then(|| hit.map(|target| target.node_id))
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
        self.resize_anchor = None;
        let logical_delta = delta_y / self.viewport.device_scale_factor;
        self.scroll_y = (self.scroll_y + logical_delta).clamp(0.0, self.max_scroll());
        self.render_frame()
    }

    fn activate_target_at(&mut self, x: f32, y: f32) -> Result<(), HtmlBrowserError> {
        let hit = self.hit_target_at(x, y).cloned();
        tracing::debug!(
            physical_x = x,
            physical_y = y,
            pressed_target = ?self.pressed_target,
            target = ?hit,
            "HTML pointer up hit-test"
        );
        let Some(target) = hit else {
            if self.pressed_target.is_none() {
                self.blur_focused_input()?;
            }
            return Ok(());
        };
        if self.pressed_target != Some(target.node_id) {
            return Ok(());
        }
        if !matches!(target.kind, HitTargetKind::Input) {
            self.blur_focused_input()?;
        }
        match target.kind {
            HitTargetKind::Input => self.focus_input(target.node_id),
            HitTargetKind::Summary { details_node_id } => {
                self.toggle_details(target.node_id, details_node_id)
            }
            HitTargetKind::Click => self.dispatch_click(target.node_id),
        }
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
        self.dirty_inputs.insert(node_id);
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

    pub(super) fn dispatch_runtime_event(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<(), HtmlBrowserError> {
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
        if self
            .source
            .origin
            .is_same_document_fragment_navigation(&url)
        {
            self.apply_fragment_navigation(url)?;
        } else {
            self.pending_navigation = Some(HtmlBrowserNavigationEvent { url });
        }
        Ok(())
    }

    pub(super) fn apply_fragment_navigation(
        &mut self,
        url: super::super::html_browser::HtmlBrowserOrigin,
    ) -> Result<(), HtmlBrowserError> {
        let fragment = url
            .url()
            .fragment()
            .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned());
        let (next_scroll, resize_anchor) = match fragment.as_deref() {
            None | Some("") => (0.0, None),
            Some(fragment) => match self.layout()?.anchor_positions.get(fragment).copied() {
                Some(position) => (position, Some(fragment.to_string())),
                None => (self.scroll_y, None),
            },
        };
        self.source.origin = url;
        self.scroll_y = next_scroll.clamp(0.0, self.max_scroll());
        self.resize_anchor = resize_anchor;
        Ok(())
    }
}

fn navigation_error(error: String) -> HtmlBrowserError {
    if error.starts_with("resource URL is invalid") {
        return runtime_failure(format!("link target is invalid: {error}"));
    }
    runtime_failure(error)
}
