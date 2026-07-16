use super::trace;
use crate::HtmlBrowserViewport;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use headless_chrome::protocol::cdp::Page;
use image::RgbaImage;

pub(super) fn capture_viewport_png(
    tab: &headless_chrome::Tab,
    _viewport: HtmlBrowserViewport,
) -> Result<Vec<u8>, String> {
    trace::stage("page:screenshot:activate");
    activate_capture_surface(tab)?;
    trace::stage("page:screenshot:cdp-capture");
    let data = tab
        .call_method(Page::CaptureScreenshot {
            format: Some(Page::CaptureScreenshotFormatOption::Png),
            quality: None,
            clip: None,
            from_surface: Some(true),
            capture_beyond_viewport: None,
            optimize_for_speed: None,
        })
        .map_err(string_error)?
        .data;
    BASE64.decode(data).map_err(string_error)
}

fn activate_capture_surface(tab: &headless_chrome::Tab) -> Result<(), String> {
    tab.activate().map_err(string_error)?;
    tab.bring_to_front().map(|_| ()).map_err(string_error)
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

pub(super) fn crop_frame_to_viewport(
    image: RgbaImage,
    viewport: HtmlBrowserViewport,
) -> Result<RgbaImage, String> {
    let (image_width, image_height) = image.dimensions();
    if image_width < viewport.width || image_height < viewport.height {
        return validate_frame_dimensions(image_width, image_height, viewport).map(|()| image);
    }
    if image_width == viewport.width && image_height == viewport.height {
        return Ok(image);
    }
    let cropped =
        image::imageops::crop_imm(&image, 0, 0, viewport.width, viewport.height).to_image();
    validate_frame_dimensions(cropped.width(), cropped.height(), viewport)?;
    Ok(cropped)
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
    fn crop_frame_to_viewport_accepts_exact_viewport_size() {
        let viewport = must(HtmlBrowserViewport::new(2, 3, 1.0));
        let image = RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]));
        let cropped = must(crop_frame_to_viewport(image, viewport));

        assert_eq!(cropped.dimensions(), (2, 3));
        assert_eq!(cropped.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn crop_frame_to_viewport_crops_larger_browser_view() {
        let viewport = must(HtmlBrowserViewport::new(2, 3, 1.0));
        let mut image = RgbaImage::from_pixel(4, 5, image::Rgba([9, 9, 9, 255]));
        image.put_pixel(1, 2, image::Rgba([17, 34, 51, 255]));
        image.put_pixel(3, 4, image::Rgba([68, 85, 102, 255]));
        let cropped = must(crop_frame_to_viewport(image, viewport));

        assert_eq!(cropped.dimensions(), (2, 3));
        assert_eq!(cropped.get_pixel(1, 2).0, [17, 34, 51, 255]);
        assert!(!cropped.pixels().any(|pixel| pixel.0 == [68, 85, 102, 255]));
    }

    #[test]
    fn crop_frame_to_viewport_rejects_smaller_browser_view() {
        let viewport = must(HtmlBrowserViewport::new(4, 5, 1.0));
        let image = RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]));
        let error = must(
            crop_frame_to_viewport(image, viewport)
                .err()
                .ok_or("undersized browser view was accepted"),
        );

        assert!(error.contains("2x3"));
        assert!(error.contains("4x5"));
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
