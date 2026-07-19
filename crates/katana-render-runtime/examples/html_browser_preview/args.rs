use super::paths::{AppResult, PreviewPaths};
use std::path::{Path, PathBuf};
use url::Url;

const DEFAULT_WIDTH: u32 = 960;
const DEFAULT_HEIGHT: u32 = 720;
const DEFAULT_DEVICE_SCALE_FACTOR: f32 = 1.0;

const HELP_OPTIONS: &str = "\
options:
  --input <path>   HTML file to render
  --out <path>     PNG output path
  --report <path>  self-contained HTML report path
  --width <px>     viewport width, default 960
  --height <px>    viewport height, default 720
  --scale <n>      device scale factor, default 1";

pub(crate) struct PreviewArgs {
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
    pub(crate) report: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) device_scale_factor: f32,
}

impl PreviewArgs {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = String>) -> AppResult<Self> {
        let mut args = Self::defaults()?;
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            args.parse_argument(argument, &mut iterator)?;
        }
        Ok(args)
    }

    fn parse_argument(
        &mut self,
        argument: String,
        iterator: &mut impl Iterator<Item = String>,
    ) -> AppResult<()> {
        match argument.as_str() {
            "--input" => self.input = next_path(iterator, "--input")?,
            "--out" => self.output = next_path(iterator, "--out")?,
            "--report" => self.report = next_path(iterator, "--report")?,
            "--width" => self.width = next_u32(iterator, "--width")?,
            "--height" => self.height = next_u32(iterator, "--height")?,
            "--scale" => self.device_scale_factor = next_f32(iterator, "--scale")?,
            "--help" | "-h" => return Err(help_text()),
            unknown => return Err(format!("unknown argument: {unknown}\n\n{}", help_text())),
        }
        Ok(())
    }

    fn defaults() -> AppResult<Self> {
        Ok(Self {
            input: PreviewPaths::default_input_path(),
            output: PreviewPaths::default_output_path()?,
            report: PreviewPaths::default_report_path()?,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            device_scale_factor: DEFAULT_DEVICE_SCALE_FACTOR,
        })
    }

    pub(crate) fn validate(&self) -> AppResult<()> {
        if !self.input.is_file() {
            return Err(format!(
                "input HTML file is missing: {}",
                self.input.display()
            ));
        }
        Ok(())
    }

    pub(crate) fn origin(&self) -> AppResult<Url> {
        let canonical = self
            .input
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize {}: {error}", self.input.display()))?;
        Url::from_file_path(&canonical)
            .map_err(|_| format!("failed to build file URL for {}", canonical.display()))
    }

    pub(crate) fn output_for(&self, suffix: &str) -> PathBuf {
        let directory = self.output.parent().unwrap_or_else(|| Path::new(""));
        let stem = self
            .output
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("html-browser-preview");
        directory.join(format!("{stem}-{suffix}.png"))
    }
}

fn next_path(iterator: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<PathBuf> {
    iterator
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a path"))
}

fn next_u32(iterator: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<u32> {
    let value = iterator
        .next()
        .ok_or_else(|| format!("{flag} requires an integer"))?;
    value
        .parse::<u32>()
        .map_err(|error| format!("invalid {flag} value {value}: {error}"))
}

fn next_f32(iterator: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<f32> {
    let value = iterator
        .next()
        .ok_or_else(|| format!("{flag} requires a number"))?;
    value
        .parse::<f32>()
        .map_err(|error| format!("invalid {flag} value {value}: {error}"))
}

fn help_text() -> String {
    format!(
        "usage: rtk cargo run -p katana-render-runtime --example html_browser_preview -- [options]\n\n\
         {HELP_OPTIONS}\n\n{}",
        default_paths_help(),
    )
}

fn default_paths_help() -> String {
    format!(
        "defaults:\n  input:  {}\n  out:    {}\n  report: {}",
        PreviewPaths::default_input_path().display(),
        PreviewPaths::fallback_output_path().display(),
        PreviewPaths::fallback_report_path().display(),
    )
}
