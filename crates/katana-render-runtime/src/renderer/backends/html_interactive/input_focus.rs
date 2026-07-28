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
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        self.commit_focused_input()?;
        self.focused_input = None;
        self.dispatch_runtime_event(HtmlRuntimeEvent::Blur {
            target: HtmlNodeId(node_id),
        })
    }

    pub(super) fn dispatch_key_down(&mut self, key: String) -> Result<(), HtmlBrowserError> {
        let focused_input = self.focused_input;
        focused_input
            .or_else(|| self.runtime.body_node().map(|node| node.0))
            .map_or(Ok(()), |node_id| {
                self.dispatch_runtime_event(HtmlRuntimeEvent::KeyDown {
                    target: HtmlNodeId(node_id),
                    key: key.clone(),
                })
            })?;
        if key == "Enter" && focused_input.is_some() {
            self.commit_focused_input()?;
        }
        Ok(())
    }

    pub(super) fn dispatch_key_up(&mut self, key: String) -> Result<(), HtmlBrowserError> {
        self.focused_input
            .or_else(|| self.runtime.body_node().map(|node| node.0))
            .map_or(Ok(()), |node_id| {
                self.dispatch_runtime_event(HtmlRuntimeEvent::KeyUp {
                    target: HtmlNodeId(node_id),
                    key,
                })
            })
    }

    fn commit_focused_input(&mut self) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        if self.dirty_inputs.remove(&node_id) {
            return self.dispatch_runtime_event(HtmlRuntimeEvent::Change {
                target: HtmlNodeId(node_id),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlInteractiveSession;
    use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport};

    fn start_minimal_session() -> Result<HtmlInteractiveSession, super::super::HtmlBrowserError> {
        HtmlBrowserSource::new("<p>focus coverage</p>", "https://example.com/focus.html").and_then(
            |source| {
                HtmlBrowserViewport::new(320, 240, 1.0)
                    .and_then(|viewport| HtmlInteractiveSession::start(source, viewport))
            },
        )
    }

    #[test]
    fn commit_focused_input_is_noop_when_not_focusing() {
        let session = start_minimal_session();
        assert!(session.is_ok());
        session.into_iter().for_each(|mut session| {
            assert_eq!(session.focused_input, None);
            assert!(session.commit_focused_input().is_ok());
            assert_eq!(session.focused_input, None);
        });
    }

    #[test]
    fn dispatch_key_down_for_unfocused_input_does_not_try_to_commit() {
        let session = start_minimal_session();
        assert!(session.is_ok());
        session.into_iter().for_each(|mut session| {
            assert_eq!(session.focused_input, None);
            assert!(session.dispatch_key_down("Enter".to_string()).is_ok());
            assert_eq!(session.focused_input, None);
        });
    }

    #[test]
    fn dispatch_key_down_propagates_listener_errors() {
        let source = HtmlBrowserSource::new(
            "<body><script>document.addEventListener('keydown', () => { throw new Error('keydown failed'); });</script></body>",
            "https://example.com/focus.html",
        );
        let viewport = HtmlBrowserViewport::new(320, 240, 1.0);
        assert!(source.is_ok());
        assert!(viewport.is_ok());
        let session = source.and_then(|source| {
            viewport.and_then(|viewport| HtmlInteractiveSession::start(source, viewport))
        });
        assert!(session.is_ok());
        session.into_iter().for_each(|mut session| {
            assert!(matches!(
                session.dispatch_key_down("ArrowRight".to_string()),
                Err(error) if error.to_string().contains("keydown failed")
            ));
        });
    }

    #[test]
    fn commit_focused_input_propagates_change_listener_errors() {
        let source = HtmlBrowserSource::new(
            "<input id=target><script>document.getElementById('target').addEventListener('change', () => { throw new Error('change failed'); });</script>",
            "https://example.com/focus.html",
        );
        let viewport = HtmlBrowserViewport::new(320, 240, 1.0);
        assert!(source.is_ok());
        assert!(viewport.is_ok());
        let session = source.and_then(|source| {
            viewport.and_then(|viewport| HtmlInteractiveSession::start(source, viewport))
        });
        assert!(session.is_ok());
        session.into_iter().for_each(|mut session| {
            let node_id = session.runtime.node_for_element_id("target");
            assert!(node_id.is_some());
            node_id.into_iter().for_each(|node_id| {
                session.focused_input = Some(node_id.0);
                session.dirty_inputs.insert(node_id.0);
                assert!(matches!(
                    session.commit_focused_input(),
                    Err(error) if error.to_string().contains("change failed")
                ));
            });
        });
    }
}
