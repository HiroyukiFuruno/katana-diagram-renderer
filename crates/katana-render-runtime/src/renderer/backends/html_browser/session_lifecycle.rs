use super::super::{HtmlBrowserCommand, HtmlBrowserError, HtmlBrowserProcess, HtmlBrowserResponse};
use super::HtmlBrowserSession;

impl HtmlBrowserSession {
    pub fn recover_process(&mut self) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        let config = self
            .process_config
            .clone()
            .ok_or(HtmlBrowserError::EngineNotStarted)?;
        self.process = None;
        self.latest_frame = None;
        self.frame_update_pending = false;
        self.pending_navigation = None;
        let mut process = HtmlBrowserProcess::spawn(&config)?;
        let response = process.request(HtmlBrowserCommand::Load {
            source: self.source.clone(),
            viewport: self.viewport,
        })?;
        self.accept_response(response)?;
        self.process = Some(process);
        Ok(())
    }

    fn process_mut(&mut self) -> Result<&mut HtmlBrowserProcess, HtmlBrowserError> {
        self.process
            .as_mut()
            .ok_or(HtmlBrowserError::EngineNotStarted)
    }

    pub(super) fn request_process(
        &mut self,
        command: HtmlBrowserCommand,
    ) -> Result<HtmlBrowserResponse, HtmlBrowserError> {
        let result = self.process_mut()?.request(command);
        if result.as_ref().is_err_and(Self::drops_process_after_error) {
            self.process = None;
        }
        result
    }

    #[rustfmt::skip]
    pub(super) fn drops_process_after_error(error: &HtmlBrowserError) -> bool {
        matches!(error, HtmlBrowserError::InvalidProcessMessage { .. } | HtmlBrowserError::ProcessWrite { .. } | HtmlBrowserError::ProcessRead { .. } | HtmlBrowserError::ProcessTimeout { .. } | HtmlBrowserError::ProcessCrashed { .. })
    }
}
