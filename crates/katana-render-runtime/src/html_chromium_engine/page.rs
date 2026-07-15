use super::{
    chromium_process::{ChromiumProcess, launch_chromium},
    document, policy, runtime, source, trace,
};
use crate::{HtmlBrowserFrame, HtmlBrowserPixelFormat, HtmlBrowserViewport};
use headless_chrome::{Browser, protocol::cdp::Page, types::Bounds};
use std::{path::PathBuf, sync::Arc};

pub(super) struct ChromiumPage {
    pub(super) tab: Arc<headless_chrome::Tab>,
    _browser: Browser,
    _chromium: ChromiumProcess,
    pub(super) source: source::BrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) generation: u64,
    pub(super) focused: bool,
    pub(super) pointer_down: Option<(f32, f32, u8)>,
    temporary_document: Option<PathBuf>,
}

impl ChromiumPage {
    pub(super) fn new(
        source: source::BrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, String> {
        trace::stage("page:new:chrome-binary");
        let chrome_binary = runtime::chrome_binary_path()?;
        trace::stage("page:new:launch-chromium");
        let (browser, chromium) = launch_chromium(&chrome_binary, viewport)?;
        trace::stage("page:new:new-tab");
        let tab = browser.new_tab().map_err(string_error)?;
        let mut page = Self {
            _browser: browser,
            _chromium: chromium,
            tab,
            source,
            viewport,
            generation: 0,
            focused: true,
            pointer_down: None,
            temporary_document: None,
        };
        trace::stage("page:new:set-viewport");
        runtime::set_viewport(&page.tab, viewport)?;
        trace::stage("page:new:load");
        page.load()?;
        trace::stage("page:new:ready");
        Ok(page)
    }

    pub(super) fn navigate(
        &mut self,
        source: source::BrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<(), String> {
        self.source = source;
        self.resize(viewport)?;
        self.load()
    }

    pub(super) fn resize(&mut self, viewport: HtmlBrowserViewport) -> Result<(), String> {
        self.tab
            .set_bounds(Bounds::Normal {
                left: None,
                top: None,
                width: Some(f64::from(viewport.width)),
                height: Some(f64::from(viewport.height)),
            })
            .map_err(string_error)?;
        runtime::set_viewport(&self.tab, viewport)?;
        self.viewport = viewport;
        Ok(())
    }

    pub(super) fn screenshot(&mut self) -> Result<HtmlBrowserFrame, String> {
        if self.focused {
            trace::stage("page:screenshot:warmup");
            self.tab
                .capture_screenshot(Page::CaptureScreenshotFormatOption::Png, None, None, true)
                .map_err(string_error)?;
        }
        trace::stage("page:screenshot:capture");
        let screenshot = self
            .tab
            .capture_screenshot(Page::CaptureScreenshotFormatOption::Png, None, None, true)
            .map_err(string_error)?;
        trace::stage("page:screenshot:decode");
        let image = image::load_from_memory(&screenshot)
            .map_err(string_error)?
            .to_rgba8();
        validate_frame_dimensions(image.width(), image.height(), self.viewport)?;
        self.generation += 1;
        HtmlBrowserFrame::new(
            self.generation,
            self.source.source.origin.clone(),
            self.viewport,
            HtmlBrowserPixelFormat::Rgba8,
            image.into_raw(),
        )
        .map_err(string_error)
    }

    fn load(&mut self) -> Result<(), String> {
        trace::stage("page:load:document-url");
        document::remove_temporary_document(&mut self.temporary_document);
        let (url, temporary_document) = document::document_url(&self.source)?;
        self.temporary_document = temporary_document;
        let temporary_document = self.temporary_document.as_deref();
        trace::stage("page:load:install-policy");
        policy::install_resource_policy(&self.tab, &self.source, temporary_document)?;
        trace::stage("page:load:navigate");
        self.tab.navigate_to(&url).map_err(string_error)?;
        trace::stage("page:load:bring-to-front");
        self.tab.bring_to_front().map_err(string_error)?;
        trace::stage("page:load:focus");
        self.emulate_focus(true)?;
        self.focused = true;
        trace::stage("page:load:synchronize");
        self.synchronize_rendering()?;
        trace::stage("page:load:ready");
        Ok(())
    }
}

fn validate_frame_dimensions(
    image_width: u32,
    image_height: u32,
    viewport: HtmlBrowserViewport,
) -> Result<(), String> {
    if image_width == viewport.width && image_height == viewport.height {
        return Ok(());
    }
    Err(format!(
        "Chromium frame dimensions {image_width}x{image_height} do not match viewport {}x{}",
        viewport.width, viewport.height
    ))
}

pub(super) fn string_error(error: impl ToString) -> String {
    error.to_string()
}

impl Drop for ChromiumPage {
    fn drop(&mut self) {
        document::remove_temporary_document(&mut self.temporary_document);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_frame_dimensions_accepts_exact_viewport_size() {
        let viewport = must(HtmlBrowserViewport::new(2, 3, 1.0));

        assert_eq!(validate_frame_dimensions(2, 3, viewport), Ok(()));
    }

    #[test]
    fn validate_frame_dimensions_rejects_mismatched_image_size() {
        let viewport = must(HtmlBrowserViewport::new(2, 3, 1.0));
        let error = must(
            validate_frame_dimensions(4, 5, viewport)
                .err()
                .ok_or("mismatched image size was accepted"),
        );

        assert!(error.contains("4x5"));
        assert!(validate_frame_dimensions(2, 5, viewport).is_err());
    }

    #[test]
    fn string_error_preserves_page_error_messages() {
        assert_eq!(string_error("page failed"), "page failed");
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom"));
    }

    #[test]
    fn must_error_branch_covers_test_value_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: String = must::<String, &str>(Err("boom"));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: HtmlBrowserViewport = must::<HtmlBrowserViewport, crate::HtmlBrowserError>(
                    Err(crate::HtmlBrowserError::InvalidViewport),
                );
            })
            .is_err()
        );
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }
}
