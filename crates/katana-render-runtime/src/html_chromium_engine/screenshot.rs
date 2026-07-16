use super::trace;
use crate::HtmlBrowserViewport;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use headless_chrome::protocol::cdp::Page;

const VIEWPORT_OFFSET_EXPRESSION: &str = "JSON.stringify((()=>{const v=window.visualViewport;return {x:v?v.pageLeft:window.scrollX,y:v?v.pageTop:window.scrollY};})())";

pub(super) fn capture_viewport_png(
    tab: &headless_chrome::Tab,
    viewport: HtmlBrowserViewport,
) -> Result<Vec<u8>, String> {
    trace::stage("page:screenshot:activate");
    activate_capture_surface(tab)?;
    trace::stage("page:screenshot:offset");
    let offset = current_viewport_offset(tab)?;
    trace::stage("page:screenshot:cdp-capture");
    let data = tab
        .call_method(Page::CaptureScreenshot {
            format: Some(Page::CaptureScreenshotFormatOption::Png),
            quality: None,
            clip: Some(viewport_capture_clip(viewport, offset)),
            from_surface: Some(true),
            capture_beyond_viewport: Some(false),
            optimize_for_speed: Some(true),
        })
        .map_err(string_error)?
        .data;
    BASE64.decode(data).map_err(string_error)
}

fn activate_capture_surface(tab: &headless_chrome::Tab) -> Result<(), String> {
    tab.activate().map_err(string_error)?;
    tab.bring_to_front().map(|_| ()).map_err(string_error)
}

fn current_viewport_offset(tab: &headless_chrome::Tab) -> Result<ViewportOffset, String> {
    let value = tab
        .evaluate(VIEWPORT_OFFSET_EXPRESSION, false)
        .map_err(string_error)?
        .value;
    viewport_offset_from_value(value)
}

fn viewport_offset_from_value(value: Option<serde_json::Value>) -> Result<ViewportOffset, String> {
    let value = value.ok_or_else(missing_viewport_offset)?;
    let json = value.as_str().ok_or_else(|| {
        format!("Chromium viewport offset value was not a JSON string: {value:?}")
    })?;
    serde_json::from_str::<ViewportOffset>(json).map_err(string_error)
}

fn missing_viewport_offset() -> String {
    "Chromium did not return viewport offset".to_string()
}

#[derive(Debug, PartialEq, serde::Deserialize)]
struct ViewportOffset {
    x: f64,
    y: f64,
}

fn viewport_capture_clip(viewport: HtmlBrowserViewport, offset: ViewportOffset) -> Page::Viewport {
    Page::Viewport {
        x: offset.x,
        y: offset.y,
        width: f64::from(viewport.width),
        height: f64::from(viewport.height),
        scale: 1.0,
    }
}

pub(super) fn validate_frame_dimensions(
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
    fn viewport_capture_clip_matches_viewport_css_pixels() {
        let viewport = must(HtmlBrowserViewport::new(2, 3, 1.0));
        let clip = viewport_capture_clip(viewport, ViewportOffset { x: 4.0, y: 5.0 });

        assert_eq!(clip.x, 4.0);
        assert_eq!(clip.y, 5.0);
        assert_eq!(clip.width, 2.0);
        assert_eq!(clip.height, 3.0);
        assert_eq!(clip.scale, 1.0);
    }

    #[test]
    fn viewport_offset_from_value_parses_json_string() {
        let offset = must(viewport_offset_from_value(Some(serde_json::json!(
            r#"{"x":4.5,"y":8.25}"#
        ))));

        assert_eq!(offset, ViewportOffset { x: 4.5, y: 8.25 });
    }

    #[test]
    fn viewport_offset_from_value_rejects_missing_value() {
        let error = must(
            viewport_offset_from_value(None)
                .err()
                .ok_or("missing offset value was accepted"),
        );

        assert_eq!(error, "Chromium did not return viewport offset");
    }

    #[test]
    fn viewport_offset_from_value_rejects_non_string_value() {
        let error = must(
            viewport_offset_from_value(Some(serde_json::json!({ "x": 1.0 })))
                .err()
                .ok_or("non-string offset value was accepted"),
        );

        assert!(error.contains("not a JSON string"));
    }

    #[test]
    fn viewport_offset_from_value_rejects_invalid_json_string() {
        let error = must(
            viewport_offset_from_value(Some(serde_json::json!("{")))
                .err()
                .ok_or("invalid offset value was accepted"),
        );

        assert!(error.contains("EOF"));
    }

    #[test]
    fn string_error_preserves_screenshot_error_messages() {
        assert_eq!(string_error("capture failed"), "capture failed");
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom"));
    }

    #[test]
    fn must_error_branch_covers_screenshot_value_types() {
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
