use super::{HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};

/// Public HTML runtime entry point for browser-equivalent interactive surfaces.
#[derive(Debug, Clone, Copy)]
pub struct HtmlRuntime;

pub type HtmlRuntimeSession = HtmlBrowserSession;

impl HtmlRuntime {
    pub fn open(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<HtmlRuntimeSession, HtmlBrowserError> {
        HtmlBrowserSession::start_in_process(source, viewport)
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
