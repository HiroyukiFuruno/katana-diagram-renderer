use super::{HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserSession};

struct InteractiveUpdates {
    frame: Option<HtmlBrowserFrame>,
    navigation: Option<HtmlBrowserNavigationEvent>,
}

impl HtmlBrowserSession {
    pub(super) fn sync_interactive_state(&mut self) -> Result<(), HtmlBrowserError> {
        let updates = self.take_interactive_updates()?;
        self.sync_interactive_frame(updates.frame)?;
        if updates.navigation.is_some() {
            self.pending_navigation = updates.navigation;
        }
        Ok(())
    }

    fn take_interactive_updates(&mut self) -> Result<InteractiveUpdates, HtmlBrowserError> {
        let (frame, navigation) = {
            let interactive = self
                .interactive
                .as_mut()
                .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
            (
                interactive.latest_frame().cloned(),
                interactive.take_navigation(),
            )
        };
        Ok(InteractiveUpdates { frame, navigation })
    }

    pub(super) fn sync_interactive_frame(
        &mut self,
        frame: Option<HtmlBrowserFrame>,
    ) -> Result<(), HtmlBrowserError> {
        if let Some(frame) = frame
            && self
                .latest_frame
                .as_ref()
                .is_none_or(|latest| latest.generation != frame.generation)
        {
            self.accept_interactive_frame(frame)?;
        }
        Ok(())
    }

    fn accept_interactive_frame(
        &mut self,
        frame: HtmlBrowserFrame,
    ) -> Result<(), HtmlBrowserError> {
        let previous_origin = self.source.origin.clone();
        if previous_origin.is_same_document_fragment_navigation(&frame.origin) {
            self.source.origin = frame.origin.clone();
        }
        match self.accept_frame(frame) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.source.origin = previous_origin;
                Err(error)
            }
        }
    }
}
