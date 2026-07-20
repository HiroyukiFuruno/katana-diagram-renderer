use super::super::html_runtime::{HtmlNodeId, HtmlRuntimeEvent};
use super::{HtmlBrowserError, HtmlInteractiveSession};

impl HtmlInteractiveSession {
    pub(super) fn update_focus(&mut self, focused: bool) -> Result<(), HtmlBrowserError> {
        if !focused {
            self.blur_focused_input()?;
        }
        Ok(())
    }

    pub(super) fn focus_input(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        if self.focused_input == Some(node_id) {
            return Ok(());
        }
        self.blur_focused_input()?;
        self.focused_input = Some(node_id);
        self.input_values.entry(node_id).or_default();
        self.dispatch_runtime_event(HtmlRuntimeEvent::Focus {
            target: HtmlNodeId(node_id),
        })
    }

    pub(super) fn blur_focused_input(&mut self) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input.take() else {
            return Ok(());
        };
        if self.dirty_inputs.remove(&node_id) {
            self.dispatch_runtime_event(HtmlRuntimeEvent::Change {
                target: HtmlNodeId(node_id),
            })?;
        }
        self.dispatch_runtime_event(HtmlRuntimeEvent::Blur {
            target: HtmlNodeId(node_id),
        })
    }

    pub(super) fn dispatch_key_down(&mut self, key: String) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        self.dispatch_runtime_event(HtmlRuntimeEvent::KeyDown {
            target: HtmlNodeId(node_id),
            key,
        })
    }

    pub(super) fn dispatch_key_up(&mut self, key: String) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        self.dispatch_runtime_event(HtmlRuntimeEvent::KeyUp {
            target: HtmlNodeId(node_id),
            key,
        })
    }
}
