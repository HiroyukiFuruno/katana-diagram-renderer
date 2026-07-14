#[path = "html_browser_preview/args.rs"]
mod args;
#[path = "html_browser_preview/paths.rs"]
mod paths;
#[path = "html_browser_preview/report.rs"]
mod report;

use args::PreviewArgs;
use katana_render_runtime::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserPixelFormat, HtmlBrowserProcessConfig,
    HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};
use paths::{AppResult, PreviewPaths};
use report::{PreviewCapture, PreviewReport};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const ACCORDION_X: f32 = 250.0;
const ACCORDION_Y: f32 = 296.0;
const ACTION_X: f32 = 175.0;
const ACTION_Y: f32 = 360.0;
const FORM_X: f32 = 175.0;
const FORM_Y: f32 = 438.0;
const CLICK_INPUT_COUNT: usize = 3;
const LINK_PROBE_X: f32 = 24.0;
const LINK_PROBE_Y: f32 = 24.0;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let args = PreviewArgs::parse(env::args().skip(1))?;
    args.validate()?;

    let raw_html = fs::read_to_string(&args.input)
        .map_err(|error| format!("failed to read {}: {error}", args.input.display()))?;
    let origin = args.origin()?;
    let source =
        HtmlBrowserSource::new(raw_html, origin.as_str()).map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(args.width, args.height, args.device_scale_factor)
        .map_err(|error| error.to_string())?;
    let config = args.process_config();
    let mut session = HtmlRuntime
        .open(source, viewport, &config)
        .map_err(|error| error.to_string())?;
    let mut captures = Vec::new();
    capture_latest(&mut captures, "Initial render", &args.output, &session)?;
    run_interaction_scenario(&args, &mut session, &mut captures)?;
    if let Err(error) = session.close() {
        eprintln!("warning: failed to close browser session cleanly: {error}");
    }
    let navigation = run_link_navigation_probe(origin.as_str(), viewport, &config)?;
    PreviewReport::write(&args, origin.as_str(), &captures, navigation.as_deref())?;

    println!("png={}", args.output.display());
    println!("report={}", args.report.display());
    for capture in &captures {
        println!("capture:{}={}", capture.label, capture.path.display());
    }
    if let Some(url) = navigation {
        println!("link_navigation={url}");
    }
    Ok(())
}

fn run_interaction_scenario(
    args: &PreviewArgs,
    session: &mut HtmlBrowserSession,
    captures: &mut Vec<PreviewCapture>,
) -> AppResult<()> {
    eprintln!("interaction: accordion click");
    click(session, ACCORDION_X, ACCORDION_Y)?;
    capture_latest(
        captures,
        "Accordion opened by click",
        &args.output_for("accordion"),
        session,
    )?;
    eprintln!("interaction: action button click");
    click(session, ACTION_X, ACTION_Y)?;
    capture_latest(
        captures,
        "Button click updated DOM",
        &args.output_for("button"),
        session,
    )?;
    eprintln!("interaction: text input");
    click(session, FORM_X, FORM_Y)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ok".to_string(),
        })
        .map_err(|error| error.to_string())?;
    capture_latest(
        captures,
        "Text input delivered",
        &args.output_for("typed"),
        session,
    )
}

fn run_link_navigation_probe(
    origin: &str,
    viewport: HtmlBrowserViewport,
    config: &HtmlBrowserProcessConfig,
) -> AppResult<Option<String>> {
    eprintln!("interaction: link navigation probe");
    let source =
        HtmlBrowserSource::new(link_probe_html(), origin).map_err(|error| error.to_string())?;
    let mut session = HtmlRuntime
        .open(source, viewport, config)
        .map_err(|error| error.to_string())?;
    click(&mut session, LINK_PROBE_X, LINK_PROBE_Y)?;
    let navigation = session
        .take_navigation()
        .map(|navigation| navigation.url.as_str().to_string());
    if let Err(error) = session.close() {
        eprintln!("warning: failed to close link probe cleanly: {error}");
    }
    Ok(navigation)
}

fn link_probe_html() -> &'static str {
    r#"<!doctype html><style>html,body,a{margin:0;width:100%;height:100%;display:block}</style><a href="linked-page.html">Open linked page</a>"#
}

fn click(session: &mut HtmlBrowserSession, x: f32, y: f32) -> AppResult<()> {
    for input in click_inputs(x, y) {
        session
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn click_inputs(x: f32, y: f32) -> [HtmlBrowserInput; CLICK_INPUT_COUNT] {
    [
        HtmlBrowserInput::PointerMove { x, y },
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ]
}

fn capture_latest(
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
