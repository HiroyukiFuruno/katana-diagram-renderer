use super::{
    chromium_process::ChromiumProcess,
    document,
    navigation::NavigationMonitor,
    page_startup::open_browser_page,
    policy,
    popup_guard::PopupGuard,
    runtime,
    screenshot::{capture_viewport_png, crop_frame_to_viewport},
    source, trace,
};
use crate::{HtmlBrowserFrame, HtmlBrowserPixelFormat, HtmlBrowserViewport};
use headless_chrome::{Browser, types::Bounds};
use std::sync::Arc;

pub(super) struct ChromiumPage {
    pub(super) tab: Arc<headless_chrome::Tab>,
    _browser: Browser,
    _chromium: ChromiumProcess,
    pub(super) source: source::BrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) generation: u64,
    pub(super) focused: bool,
    pub(super) pointer_down: Option<(f32, f32, u8)>,
    pub(super) navigation: NavigationMonitor,
    pub(super) popup_guard: PopupGuard,
}

impl ChromiumPage {
    pub(super) fn new(
        source: source::BrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, String> {
        let parts = open_browser_page(viewport)?;
        let mut page = Self {
            _browser: parts.browser,
            _chromium: parts.chromium,
            tab: parts.tab,
            source,
            viewport,
            generation: 0,
            focused: true,
            pointer_down: None,
            navigation: parts.navigation,
            popup_guard: parts.popup_guard,
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
        set_view_bounds(&self.tab, viewport)?;
        runtime::set_viewport(&self.tab, viewport)?;
        self.viewport = viewport;
        Ok(())
    }

    pub(super) fn screenshot(&mut self) -> Result<HtmlBrowserFrame, String> {
        trace::stage("page:screenshot:synchronize");
        self.synchronize_rendering()?;
        let image = self.capture_image()?;
        let image = self.grow_capture_surface_if_needed(image)?;
        let image = crop_frame_to_viewport(image, self.viewport)?;
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

    fn capture_image(&self) -> Result<image::RgbaImage, String> {
        trace::stage("page:screenshot:capture");
        let screenshot = capture_viewport_png(&self.tab, self.viewport)?;
        trace::stage("page:screenshot:decode");
        image::load_from_memory(&screenshot)
            .map_err(string_error)
            .map(|image| image.to_rgba8())
    }

    fn grow_capture_surface_if_needed(
        &mut self,
        image: image::RgbaImage,
    ) -> Result<image::RgbaImage, String> {
        let dimensions = image.dimensions();
        if surface_contains_viewport(dimensions, self.viewport) {
            return Ok(image);
        }
        trace::stage("page:screenshot:grow-surface");
        grow_view_bounds(&self.tab, dimensions, self.viewport)?;
        runtime::set_viewport(&self.tab, self.viewport)?;
        self.synchronize_rendering()?;
        self.capture_image()
    }

    fn load(&mut self) -> Result<(), String> {
        trace::stage("page:load:document-url");
        let url = document::document_url(&self.source)?;
        trace::stage("page:load:install-policy");
        policy::install_resource_policy(&self.tab, &self.source, &self.navigation)?;
        trace::stage("page:load:navigate");
        self.tab.navigate_to(&url).map_err(string_error)?;
        trace::stage("page:load:bring-to-front");
        self.tab.bring_to_front().map_err(string_error)?;
        trace::stage("page:load:focus");
        self.emulate_focus(true)?;
        self.focused = true;
        trace::stage("page:load:synchronize");
        self.synchronize_loaded_rendering(&url)?;
        trace::stage("page:load:ready");
        Ok(())
    }
}

pub(super) fn string_error(error: impl ToString) -> String {
    error.to_string()
}

fn set_view_bounds(
    tab: &headless_chrome::Tab,
    viewport: HtmlBrowserViewport,
) -> Result<(), String> {
    tab.set_bounds(Bounds::Normal {
        left: None,
        top: None,
        width: Some(f64::from(viewport.width)),
        height: Some(f64::from(viewport.height)),
    })
    .map(|_| ())
    .map_err(string_error)
}

fn grow_view_bounds(
    tab: &headless_chrome::Tab,
    surface: (u32, u32),
    viewport: HtmlBrowserViewport,
) -> Result<(), String> {
    let bounds = tab.get_bounds().map_err(string_error)?;
    let missing = missing_surface_extent(surface, viewport);
    tab.set_bounds(Bounds::Normal {
        left: None,
        top: None,
        width: Some(bounds.width + f64::from(missing.0)),
        height: Some(bounds.height + f64::from(missing.1)),
    })
    .map(|_| ())
    .map_err(string_error)
}

fn surface_contains_viewport(surface: (u32, u32), viewport: HtmlBrowserViewport) -> bool {
    surface.0 >= viewport.width && surface.1 >= viewport.height
}

fn missing_surface_extent(surface: (u32, u32), viewport: HtmlBrowserViewport) -> (u32, u32) {
    (
        viewport.width.saturating_sub(surface.0),
        viewport.height.saturating_sub(surface.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_error_preserves_page_error_messages() {
        assert_eq!(string_error("page failed"), "page failed");
    }

    #[test]
    fn surface_growth_uses_only_the_measured_dimension_deficit() {
        let viewport = must(HtmlBrowserViewport::new(960, 720, 1.0));

        assert!(!surface_contains_viewport((960, 577), viewport));
        assert_eq!(missing_surface_extent((960, 577), viewport), (0, 143));
        assert!(surface_contains_viewport((1_024, 768), viewport));
        assert_eq!(missing_surface_extent((1_024, 768), viewport), (0, 0));
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

    #[test]
    fn must_ok_branch_covers_test_value_types() {
        assert_eq!(must::<String, &str>(Ok("ok".to_string())), "ok");
        assert_eq!(
            must(HtmlBrowserViewport::new(2, 3, 1.0)),
            HtmlBrowserViewport {
                width: 2,
                height: 3,
                device_scale_factor: 1.0,
            }
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
