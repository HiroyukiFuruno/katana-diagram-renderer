use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) type AppResult<T> = Result<T, String>;

const PREVIEW_INPUT: &str = "examples/fixtures/html_browser_preview.html";
const PREVIEW_OUTPUT: &str = "tmp/html-browser-preview/krr-html-browser-preview.png";
const PREVIEW_REPORT: &str = "tmp/html-browser-preview/index.html";

pub(crate) struct PreviewPaths;

impl PreviewPaths {
    pub(crate) fn create_parent_dir(path: &Path) -> AppResult<()> {
        match path.parent() {
            Some(parent) => fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display())),
            None => Ok(()),
        }
    }

    pub(crate) fn default_input_path() -> PathBuf {
        Self::package_root().join(PREVIEW_INPUT)
    }

    pub(crate) fn default_output_path() -> AppResult<PathBuf> {
        Self::workspace_path(PREVIEW_OUTPUT)
    }

    pub(crate) fn default_report_path() -> AppResult<PathBuf> {
        Self::workspace_path(PREVIEW_REPORT)
    }

    pub(crate) fn fallback_output_path() -> PathBuf {
        Self::fallback_workspace_path(PREVIEW_OUTPUT)
    }

    pub(crate) fn fallback_report_path() -> PathBuf {
        Self::fallback_workspace_path(PREVIEW_REPORT)
    }

    fn package_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn workspace_root() -> AppResult<PathBuf> {
        Self::package_root()
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve workspace root from CARGO_MANIFEST_DIR".to_string())
    }

    fn workspace_path(relative: impl AsRef<Path>) -> AppResult<PathBuf> {
        Ok(Self::workspace_root()?.join(relative))
    }

    fn fallback_workspace_path(relative: impl AsRef<Path>) -> PathBuf {
        Self::workspace_path(relative.as_ref()).unwrap_or_else(|_| relative.as_ref().to_path_buf())
    }
}
