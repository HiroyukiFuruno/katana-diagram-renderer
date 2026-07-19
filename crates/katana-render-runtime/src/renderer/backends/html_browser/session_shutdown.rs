use super::{HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSessionState};

impl HtmlBrowserSession {
    pub fn close(&mut self) -> Result<(), HtmlBrowserError> {
        if self.state == HtmlBrowserSessionState::Closed {
            return Ok(());
        }
        self.state = HtmlBrowserSessionState::Closed;
        self.interactive = None;
        Ok(())
    }
}

impl Drop for HtmlBrowserSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
