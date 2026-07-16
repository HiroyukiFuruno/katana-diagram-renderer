use super::trace;
use crate::HtmlBrowserViewport;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use headless_chrome::{
    browser::tab::EventListener,
    protocol::cdp::{Page, types::Event},
};
use image::RgbaImage;
use std::{
    sync::{Arc, Weak, mpsc},
    time::Duration,
};

const SCREENCAST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);
type ScreencastListener = dyn EventListener<Event> + Send + Sync;
type ScreencastListenerHandle = Weak<ScreencastListener>;
type ScreencastReceiver = mpsc::Receiver<ScreencastFrameCapture>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreencastFrameCapture {
    data: String,
    session_id: u32,
}

pub(super) fn capture_viewport_png(
    tab: &headless_chrome::Tab,
    _viewport: HtmlBrowserViewport,
) -> Result<Vec<u8>, String> {
    trace::stage("page:screenshot:activate");
    activate_capture_surface(tab)?;
    let (receiver, listener) = install_screencast_listener(tab)?;
    start_screencast(tab)?;
    let frame = receive_screencast_frame(&receiver);
    let frame = finish_screencast(tab, &listener, frame)?;
    trace::stage("page:screenshot:screencast-decode");
    decode_screencast_frame(frame)
}

fn install_screencast_listener(
    tab: &headless_chrome::Tab,
) -> Result<(ScreencastReceiver, ScreencastListenerHandle), String> {
    trace::stage("page:screenshot:screencast-listen");
    let (sender, receiver) = mpsc::channel();
    let listener = tab
        .add_event_listener(Arc::new(move |event: &Event| {
            if let Event::PageScreencastFrame(frame) = event {
                let _ = sender.send(ScreencastFrameCapture {
                    data: frame.params.data.clone(),
                    session_id: frame.params.session_id,
                });
            }
        }))
        .map_err(string_error)?;
    Ok((receiver, listener))
}

fn start_screencast(tab: &headless_chrome::Tab) -> Result<(), String> {
    trace::stage("page:screenshot:screencast-start");
    tab.start_screencast(
        Some(Page::StartScreencastFormatOption::Png),
        None,
        None,
        None,
        Some(1),
    )
    .map_err(string_error)
}

fn receive_screencast_frame(
    receiver: &mpsc::Receiver<ScreencastFrameCapture>,
) -> Result<ScreencastFrameCapture, String> {
    trace::stage("page:screenshot:screencast-frame");
    receiver
        .recv_timeout(SCREENCAST_FRAME_TIMEOUT)
        .map_err(screencast_timeout_error)
}

fn finish_screencast(
    tab: &headless_chrome::Tab,
    listener: &ScreencastListenerHandle,
    frame: Result<ScreencastFrameCapture, String>,
) -> Result<ScreencastFrameCapture, String> {
    let ack_result = frame
        .as_ref()
        .map_or(Ok(()), |frame| tab.ack_screencast(frame.session_id))
        .map_err(string_error);
    trace::stage("page:screenshot:screencast-stop");
    let stop_result = tab.stop_screencast().map_err(string_error);
    let remove_result = tab.remove_event_listener(listener).map_err(string_error);
    stop_result?;
    remove_result?;
    let frame = frame?;
    ack_result?;
    Ok(frame)
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

fn decode_screencast_frame(frame: ScreencastFrameCapture) -> Result<Vec<u8>, String> {
    BASE64.decode(frame.data).map_err(string_error)
}

fn screencast_timeout_error(error: mpsc::RecvTimeoutError) -> String {
    match error {
        mpsc::RecvTimeoutError::Timeout => "Chromium screencast frame timed out".to_string(),
        mpsc::RecvTimeoutError::Disconnected => {
            "Chromium screencast frame channel disconnected".to_string()
        }
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
    fn decode_screencast_frame_decodes_base64_png() {
        assert_eq!(
            must(decode_screencast_frame(ScreencastFrameCapture {
                data: "AQID".to_string(),
                session_id: 7,
            })),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn decode_screencast_frame_rejects_invalid_base64() {
        let error = must(
            decode_screencast_frame(ScreencastFrameCapture {
                data: "not base64".to_string(),
                session_id: 7,
            })
            .err()
            .ok_or("invalid screencast frame was accepted"),
        );

        assert!(error.contains("Invalid"));
    }

    #[test]
    fn screencast_timeout_error_preserves_channel_state() {
        assert_eq!(
            screencast_timeout_error(mpsc::RecvTimeoutError::Timeout),
            "Chromium screencast frame timed out"
        );
        assert_eq!(
            screencast_timeout_error(mpsc::RecvTimeoutError::Disconnected),
            "Chromium screencast frame channel disconnected"
        );
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
