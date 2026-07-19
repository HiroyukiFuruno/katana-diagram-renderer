use super::{
    paths::{AppResult, PreviewPaths},
    report::PreviewCapture,
};
use katana_render_runtime::{HtmlBrowserFrame, HtmlBrowserPixelFormat, HtmlBrowserSession};
use std::path::{Path, PathBuf};

pub(crate) struct PreviewCaptureWriter;

impl PreviewCaptureWriter {
    pub(crate) fn capture_latest(
        captures: &mut Vec<PreviewCapture>,
        label: &str,
        path: &Path,
        session: &HtmlBrowserSession,
    ) -> AppResult<()> {
        let frame = session
            .latest_frame()
            .ok_or_else(|| format!("browser session did not produce frame for {label}"))?
            .clone();
        save_frame_png(&frame, path)?;
        captures.push(PreviewCapture::new(label, PathBuf::from(path), frame));
        Ok(())
    }
}

fn save_frame_png(frame: &HtmlBrowserFrame, output: &Path) -> AppResult<()> {
    if frame.pixel_format != HtmlBrowserPixelFormat::Rgba8 {
        return Err(format!(
            "unsupported pixel format: {:?}",
            frame.pixel_format
        ));
    }
    PreviewPaths::create_parent_dir(output)?;
    image::save_buffer_with_format(
        output,
        &frame.pixels,
        frame.viewport.width,
        frame.viewport.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|error| format!("failed to write PNG {}: {error}", output.display()))
}
