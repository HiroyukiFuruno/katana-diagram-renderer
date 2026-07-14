use super::{HtmlBrowserError, HtmlBrowserOrigin, HtmlBrowserViewport};
use serde::{Deserialize, Serialize};

const RGBA8_BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlBrowserPixelFormat {
    Rgba8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlBrowserFrame {
    pub generation: u64,
    pub origin: HtmlBrowserOrigin,
    pub viewport: HtmlBrowserViewport,
    pub pixel_format: HtmlBrowserPixelFormat,
    pub pixels: Vec<u8>,
}

impl HtmlBrowserFrame {
    pub fn new(
        generation: u64,
        origin: HtmlBrowserOrigin,
        viewport: HtmlBrowserViewport,
        pixel_format: HtmlBrowserPixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, HtmlBrowserError> {
        viewport.validate()?;
        let expected_bytes = (viewport.width as usize)
            .checked_mul(viewport.height as usize)
            .and_then(|pixels| pixels.checked_mul(RGBA8_BYTES_PER_PIXEL))
            .ok_or(HtmlBrowserError::FrameDimensionsOverflow)?;
        if pixels.len() != expected_bytes {
            return Err(HtmlBrowserError::InvalidFrameBufferSize {
                actual_bytes: pixels.len(),
                expected_bytes,
            });
        }
        Ok(Self {
            generation,
            origin,
            viewport,
            pixel_format,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HtmlBrowserOrigin, HtmlBrowserPixelFormat, HtmlBrowserViewport};

    #[test]
    fn frame_rejects_invalid_buffer_sizes() {
        let origin = origin();
        let viewport = must(HtmlBrowserViewport::new(2, 2, 1.0));

        let invalid = HtmlBrowserFrame::new(
            1,
            origin.clone(),
            viewport,
            HtmlBrowserPixelFormat::Rgba8,
            vec![0; 15],
        );
        assert_eq!(
            invalid,
            Err(HtmlBrowserError::InvalidFrameBufferSize {
                actual_bytes: 15,
                expected_bytes: 16
            })
        );
    }

    #[test]
    fn frame_rejects_dimension_overflow() {
        let origin = origin();
        let overflow_viewport = HtmlBrowserViewport {
            width: u32::MAX,
            height: u32::MAX,
            device_scale_factor: 1.0,
        };
        let overflow = HtmlBrowserFrame::new(
            1,
            origin,
            overflow_viewport,
            HtmlBrowserPixelFormat::Rgba8,
            Vec::new(),
        );
        assert_eq!(overflow, Err(HtmlBrowserError::FrameDimensionsOverflow));
    }

    #[test]
    fn frame_rejects_invalid_viewport() {
        let invalid_viewport = HtmlBrowserViewport {
            width: 0,
            height: 1,
            device_scale_factor: 1.0,
        };
        let frame = HtmlBrowserFrame::new(
            1,
            origin(),
            invalid_viewport,
            HtmlBrowserPixelFormat::Rgba8,
            Vec::new(),
        );

        assert_eq!(frame, Err(HtmlBrowserError::InvalidViewport));
    }

    #[test]
    #[should_panic(
        expected = "unexpected test error: browser viewport dimensions must be non-zero"
    )]
    fn must_reports_unexpected_test_errors() {
        let _: HtmlBrowserViewport = must(HtmlBrowserViewport::new(0, 1, 1.0));
    }

    fn origin() -> HtmlBrowserOrigin {
        must(HtmlBrowserOrigin::parse("https://example.test/frame.html"))
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
