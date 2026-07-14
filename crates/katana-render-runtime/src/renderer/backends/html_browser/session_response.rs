use super::super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserResponse,
    response::{closed_response, engine_error, validate_protocol},
};
use super::HtmlBrowserSession;

impl HtmlBrowserSession {
    pub(super) fn accept_response(
        &mut self,
        response: HtmlBrowserResponse,
    ) -> Result<(), HtmlBrowserError> {
        match response {
            HtmlBrowserResponse::Frame {
                protocol_version,
                frame,
            } => self.accept_frame_response(protocol_version, frame),
            HtmlBrowserResponse::Error {
                protocol_version,
                code,
                message,
            } => engine_error(protocol_version, code, message),
            HtmlBrowserResponse::Closed { protocol_version } => closed_response(protocol_version),
            HtmlBrowserResponse::Navigation {
                protocol_version,
                navigation,
            } => self.accept_navigation_response(protocol_version, navigation),
        }
    }

    fn accept_frame_response(
        &mut self,
        version: u32,
        frame: HtmlBrowserFrame,
    ) -> Result<(), HtmlBrowserError> {
        validate_protocol(version)?;
        self.accept_frame(frame)
    }

    fn accept_navigation_response(
        &mut self,
        version: u32,
        navigation: HtmlBrowserNavigationEvent,
    ) -> Result<(), HtmlBrowserError> {
        validate_protocol(version)?;
        self.pending_navigation = Some(navigation);
        Ok(())
    }
}
