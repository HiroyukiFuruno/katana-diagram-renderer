use super::{document, main_document, policy, runtime, source};
use crate::{HtmlBrowserFrame, HtmlBrowserPixelFormat, HtmlBrowserViewport};
use headless_chrome::{
    Browser, LaunchOptionsBuilder,
    browser::tab::RequestPausedDecision,
    protocol::cdp::{Emulation, Fetch, Network, Page},
    types::Bounds,
};
use std::{fs, path::PathBuf, sync::Arc};

pub(super) struct ChromiumPage {
    pub(super) tab: Arc<headless_chrome::Tab>,
    _browser: Browser,
    pub(super) source: source::BrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) generation: u64,
    pub(super) pointer_down: Option<(f32, f32, u8)>,
    temporary_document: Option<PathBuf>,
}

impl ChromiumPage {
    pub(super) fn new(
        source: source::BrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, String> {
        let chrome_binary = runtime::chrome_binary_path()?;
        let options = LaunchOptionsBuilder::default()
            .path(Some(chrome_binary))
            .window_size(Some((viewport.width, viewport.height)))
            .args(runtime::rendering_args())
            .build()
            .map_err(string_error)?;
        let browser = Browser::new(options).map_err(string_error)?;
        let tab = browser.new_tab().map_err(string_error)?;
        tab.activate().map_err(string_error)?;
        let mut page = Self {
            _browser: browser,
            tab,
            source,
            viewport,
            generation: 0,
            pointer_down: None,
            temporary_document: None,
        };
        page.set_viewport(viewport)?;
        page.load()?;
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
        self.set_viewport(viewport)?;
        self.viewport = viewport;
        Ok(())
    }

    pub(super) fn screenshot(&mut self) -> Result<HtmlBrowserFrame, String> {
        let screenshot = self
            .tab
            .capture_screenshot(Page::CaptureScreenshotFormatOption::Png, None, None, true)
            .map_err(string_error)?;
        let image = image::load_from_memory(&screenshot)
            .map_err(string_error)?
            .to_rgba8();
        validate_frame_dimensions(image.width(), image.height(), self.viewport)?;
        self.generation += 1;
        let pixels = image.into_raw();
        HtmlBrowserFrame::new(
            self.generation,
            self.source.source.origin.clone(),
            self.viewport,
            HtmlBrowserPixelFormat::Rgba8,
            pixels,
        )
        .map_err(string_error)
    }

    fn load(&mut self) -> Result<(), String> {
        self.remove_temporary_document();
        let (url, temporary_document) = document::document_url(&self.source)?;
        self.temporary_document = temporary_document;
        self.install_resource_policy(self.temporary_document.as_deref())?;
        self.tab
            .navigate_to(&url)
            .and_then(|tab| tab.wait_until_navigated())
            .map_err(string_error)?;
        Ok(())
    }

    fn set_viewport(&self, viewport: HtmlBrowserViewport) -> Result<(), String> {
        self.tab
            .call_method(Emulation::SetDeviceMetricsOverride {
                width: viewport.width,
                height: viewport.height,
                device_scale_factor: f64::from(viewport.device_scale_factor),
                mobile: false,
                scale: None,
                screen_width: None,
                screen_height: None,
                position_x: None,
                position_y: None,
                dont_set_visible_size: None,
                screen_orientation: None,
                viewport: None,
                display_feature: None,
                device_posture: None,
            })
            .map_err(string_error)?;
        Ok(())
    }

    fn install_resource_policy(
        &self,
        temporary_document: Option<&std::path::Path>,
    ) -> Result<(), String> {
        let policy = policy::BrowserResourcePolicy::from_source_with_temporary_document(
            &self.source,
            temporary_document,
        );
        let main_document = main_document::MainDocument::from_source(&self.source);
        self.tab
            .enable_request_interception(Arc::new(
                move |_transport, _session_id, event: Fetch::events::RequestPausedEvent| {
                    request_decision(event, &main_document, &policy)
                },
            ))
            .map_err(string_error)?;
        self.tab.enable_fetch(None, None).map_err(string_error)?;
        Ok(())
    }

    fn remove_temporary_document(&mut self) {
        if let Some(path) = self.temporary_document.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn request_decision(
    event: Fetch::events::RequestPausedEvent,
    main_document: &Option<main_document::MainDocument>,
    policy: &policy::BrowserResourcePolicy,
) -> RequestPausedDecision {
    if let Some(document) = main_document
        && document.matches(&event.params.request.url)
    {
        return RequestPausedDecision::Fulfill(document.fulfill(event.params.request_id));
    }
    if policy.allows(&event.params.request.url) {
        RequestPausedDecision::Continue(None)
    } else {
        RequestPausedDecision::Fail(Fetch::FailRequest {
            request_id: event.params.request_id,
            error_reason: Network::ErrorReason::BlockedByClient,
        })
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

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

impl Drop for ChromiumPage {
    fn drop(&mut self) {
        self.remove_temporary_document();
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

    fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }
}
