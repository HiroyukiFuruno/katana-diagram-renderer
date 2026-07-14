use super::{
    HtmlBrowserCommand, HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserInput,
    HtmlBrowserNavigation, HtmlBrowserNavigationEvent, HtmlBrowserProcess,
    HtmlBrowserProcessConfig, HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport,
};

#[derive(Debug)]
pub struct HtmlBrowserSession {
    source: HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
    state: HtmlBrowserSessionState,
    latest_frame: Option<HtmlBrowserFrame>,
    frame_update_pending: bool,
    pending_navigation: Option<HtmlBrowserNavigationEvent>,
    process_config: Option<HtmlBrowserProcessConfig>,
    process: Option<HtmlBrowserProcess>,
}

impl HtmlBrowserSession {
    pub fn new(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        viewport.validate()?;
        Ok(Self {
            source,
            viewport,
            state: HtmlBrowserSessionState::Active,
            latest_frame: None,
            frame_update_pending: false,
            pending_navigation: None,
            process_config: None,
            process: None,
        })
    }

    pub fn start(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        config: &HtmlBrowserProcessConfig,
    ) -> Result<Self, HtmlBrowserError> {
        let mut session = Self::new(source, viewport)?;
        let mut process = HtmlBrowserProcess::spawn(config)?;
        let command = HtmlBrowserCommand::Load {
            source: session.source.clone(),
            viewport: session.viewport,
        };
        let response = process.request(command)?;
        session.accept_response(response)?;
        session.process_config = Some(config.clone());
        session.process = Some(process);
        Ok(session)
    }

    pub fn source(&self) -> &HtmlBrowserSource {
        &self.source
    }
    pub fn viewport(&self) -> HtmlBrowserViewport {
        self.viewport
    }
    pub fn state(&self) -> HtmlBrowserSessionState {
        self.state
    }
    pub fn latest_frame(&self) -> Option<&HtmlBrowserFrame> {
        self.latest_frame.as_ref()
    }
    pub fn take_frame_update(&mut self) -> Option<&HtmlBrowserFrame> {
        if !self.frame_update_pending {
            return None;
        }
        self.frame_update_pending = false;
        self.latest_frame.as_ref()
    }
    pub fn take_navigation(&mut self) -> Option<HtmlBrowserNavigationEvent> {
        self.pending_navigation.take()
    }
    pub fn has_process(&self) -> bool {
        self.process.is_some()
    }

    pub fn navigate(&mut self, navigation: HtmlBrowserNavigation) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        navigation.source.validate()?;
        let viewport = self.viewport;
        let command = HtmlBrowserCommand::Load {
            source: navigation.source.clone(),
            viewport,
        };
        let response = self.request_process(command)?;
        self.source = navigation.source;
        self.latest_frame = None;
        self.frame_update_pending = false;
        self.accept_response(response)
    }

    pub fn resize(&mut self, viewport: HtmlBrowserViewport) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        viewport.validate()?;
        let response = self.request_process(HtmlBrowserCommand::Resize { viewport })?;
        self.viewport = viewport;
        self.latest_frame = None;
        self.frame_update_pending = false;
        self.accept_response(response)
    }

    pub fn refresh_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        let response = self.request_process(HtmlBrowserCommand::Frame)?;
        self.accept_response(response)
    }

    pub fn dispatch_input(&mut self, input: HtmlBrowserInput) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        input.validate()?;
        let response = self.request_process(HtmlBrowserCommand::Input { input })?;
        self.accept_response(response)
    }

    pub fn close(&mut self) -> Result<(), HtmlBrowserError> {
        if self.state == HtmlBrowserSessionState::Closed {
            return Ok(());
        }
        self.state = HtmlBrowserSessionState::Closed;
        if let Some(process) = self.process.as_mut() {
            if process.request(HtmlBrowserCommand::Close).is_ok() {
                process.wait_for_exit()?;
            } else {
                process.terminate()?;
            }
        }
        self.process = None;
        Ok(())
    }

    pub fn accept_frame(&mut self, frame: HtmlBrowserFrame) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        if frame.origin != self.source.origin {
            return Err(HtmlBrowserError::FrameOriginMismatch {
                expected: self.source.origin.as_str().to_owned(),
                actual: frame.origin.as_str().to_owned(),
            });
        }
        if let Some(latest) = &self.latest_frame
            && frame.generation <= latest.generation
        {
            return Err(HtmlBrowserError::StaleFrameGeneration {
                latest: latest.generation,
                received: frame.generation,
            });
        }
        self.latest_frame = Some(frame);
        self.frame_update_pending = true;
        Ok(())
    }

    fn ensure_active(&self) -> Result<(), HtmlBrowserError> {
        (self.state == HtmlBrowserSessionState::Active)
            .then_some(())
            .ok_or(HtmlBrowserError::SessionClosed)
    }
}

impl Drop for HtmlBrowserSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[path = "session_response.rs"]
mod response_acceptance;

#[path = "session_lifecycle.rs"]
mod process_lifecycle;

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_lifecycle_tests.rs"]
mod lifecycle_tests;
