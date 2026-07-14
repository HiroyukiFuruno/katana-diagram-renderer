use super::{
    args::PreviewArgs,
    paths::{AppResult, PreviewPaths},
};
use base64::Engine as _;
use katana_render_runtime::HtmlBrowserFrame;
use std::{fs, path::PathBuf};

const REPORT_STYLE: &str = r#"body { margin: 24px; font: 15px/1.5 system-ui, sans-serif; color: #17202a; background: #f5f7fb; }
main { max-width: 1100px; margin: 0 auto; }
img { width: 100%; border: 1px solid #ccd4e0; background: white; }
figure { margin: 0 0 24px; }
figcaption { margin: 8px 0 0; font-weight: 700; }
pre { overflow: auto; padding: 16px; background: #111827; color: #e5e7eb; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 12px; }
dt { font-weight: 700; }
dd { margin: 0; }"#;

const REPORT_TEMPLATE: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>KRR HTML browser preview</title>
<style>
{{STYLE}}
</style>
<main>
<h1>KRR HTML browser preview</h1>
<p>This PNG was rendered by the KRR Chromium browser session.</p>
<dl>
<dt>Origin</dt><dd>{{ORIGIN}}</dd>
<dt>Viewport</dt><dd>{{WIDTH}} x {{HEIGHT}} @ {{SCALE}}</dd>
<dt>Frame generation</dt><dd>{{GENERATION}}</dd>
<dt>Input</dt><dd>{{INPUT}}</dd>
<dt>Helper</dt><dd>{{HELPER}}</dd>
<dt>Link navigation probe</dt><dd>{{NAVIGATION}}</dd>
</dl>
<h2>Rendered PNG captures</h2>
{{CAPTURES}}
<h2>Input HTML</h2>
<pre>{{SOURCE}}</pre>
</main>
"#;

pub(crate) struct PreviewReport;

pub(crate) struct PreviewCapture {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    frame: HtmlBrowserFrame,
}

struct ReportInputs {
    source: String,
}

impl PreviewCapture {
    pub(crate) fn new(label: &str, path: PathBuf, frame: HtmlBrowserFrame) -> Self {
        Self {
            label: label.to_string(),
            path,
            frame,
        }
    }
}

impl PreviewReport {
    pub(crate) fn write(
        args: &PreviewArgs,
        origin: &str,
        captures: &[PreviewCapture],
        navigation_url: Option<&str>,
    ) -> AppResult<()> {
        PreviewPaths::create_parent_dir(&args.report)?;
        let inputs = report_inputs(args)?;
        let report = report_document(args, origin, captures, navigation_url, &inputs)?;
        fs::write(&args.report, report)
            .map_err(|error| format!("failed to write report {}: {error}", args.report.display()))
    }
}

fn report_inputs(args: &PreviewArgs) -> AppResult<ReportInputs> {
    let source = fs::read_to_string(&args.input)
        .map_err(|error| format!("failed to read {}: {error}", args.input.display()))?;
    Ok(ReportInputs { source })
}

fn report_document(
    args: &PreviewArgs,
    origin: &str,
    captures: &[PreviewCapture],
    navigation_url: Option<&str>,
    inputs: &ReportInputs,
) -> AppResult<String> {
    let values = report_values(args, origin, captures, navigation_url, inputs)?;
    Ok(replace_report_placeholders(REPORT_TEMPLATE, &values))
}

fn report_values(
    args: &PreviewArgs,
    origin: &str,
    captures: &[PreviewCapture],
    navigation_url: Option<&str>,
    inputs: &ReportInputs,
) -> AppResult<Vec<(&'static str, String)>> {
    let frame = first_frame(captures)?;
    Ok(vec![
        ("{{STYLE}}", REPORT_STYLE.to_string()),
        ("{{ORIGIN}}", escape_html(origin)),
        ("{{WIDTH}}", frame.viewport.width.to_string()),
        ("{{HEIGHT}}", frame.viewport.height.to_string()),
        ("{{SCALE}}", frame.viewport.device_scale_factor.to_string()),
        ("{{GENERATION}}", frame.generation.to_string()),
        ("{{INPUT}}", escape_html(&args.input.display().to_string())),
        (
            "{{HELPER}}",
            escape_html(&args.helper.display().to_string()),
        ),
        (
            "{{NAVIGATION}}",
            escape_html(navigation_url.unwrap_or("no navigation event captured")),
        ),
        ("{{CAPTURES}}", capture_figures(captures)?),
        ("{{SOURCE}}", escape_html(&inputs.source)),
    ])
}

fn first_frame(captures: &[PreviewCapture]) -> AppResult<&HtmlBrowserFrame> {
    captures
        .first()
        .map(|capture| &capture.frame)
        .ok_or_else(|| "preview report requires at least one frame capture".to_string())
}

fn capture_figures(captures: &[PreviewCapture]) -> AppResult<String> {
    let mut figures = String::new();
    for capture in captures {
        figures.push_str(&capture_figure(capture)?);
    }
    Ok(figures)
}

fn capture_figure(capture: &PreviewCapture) -> AppResult<String> {
    let png = fs::read(&capture.path)
        .map_err(|error| format!("failed to read PNG {}: {error}", capture.path.display()))?;
    let encoded_png = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(format!(
        "<figure><img alt=\"{}\" src=\"data:image/png;base64,{}\"><figcaption>{} - generation {}</figcaption></figure>\n",
        escape_html(&capture.label),
        encoded_png,
        escape_html(&capture.label),
        capture.frame.generation
    ))
}

fn replace_report_placeholders(template: &str, values: &[(&str, String)]) -> String {
    let mut report = template.to_string();
    for (placeholder, value) in values {
        report = report.replace(placeholder, value);
    }
    report
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
