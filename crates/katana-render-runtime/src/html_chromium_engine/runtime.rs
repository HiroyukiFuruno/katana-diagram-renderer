use crate::HtmlBrowserViewport;
use headless_chrome::protocol::cdp::Emulation;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

const CHROMIUM_BINARY_ENV: &str = "KRR_CHROME_BIN";
const RENDERING_ARGS: [&str; 3] = [
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
];

#[cfg(test)]
pub(super) static CHROMIUM_BINARY_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) fn chrome_binary_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(CHROMIUM_BINARY_ENV).map(PathBuf::from) {
        return executable_file(path, "KRR_CHROME_BIN");
    }
    let executable = std::env::current_exe().map_err(string_error)?;
    chrome_binary_path_adjacent_to(&executable)
}

pub(super) fn chrome_binary_path_adjacent_to(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable
        .parent()
        .ok_or("KRR browser helper has no parent directory".to_string())?;
    let artifact = manifest_artifact()?;
    executable_file(
        directory
            .join("chromium")
            .join(artifact.platform)
            .join(artifact.executable),
        "KRR Chromium bundle",
    )
}

pub(super) fn rendering_args() -> Vec<&'static OsStr> {
    RENDERING_ARGS.map(OsStr::new).to_vec()
}

pub(super) fn set_viewport(
    tab: &headless_chrome::Tab,
    viewport: HtmlBrowserViewport,
) -> Result<(), String> {
    tab.call_method(Emulation::SetDeviceMetricsOverride {
        width: viewport.width,
        height: viewport.height,
        device_scale_factor: f64::from(viewport.device_scale_factor),
        mobile: false,
        scale: None,
        screen_width: None,
        screen_height: None,
        position_x: None,
        position_y: None,
        dont_set_visible_size: None,
        screen_orientation: None,
        viewport: None,
        display_feature: None,
        device_posture: None,
    })
    .map(|_| ())
    .map_err(string_error)
}

#[derive(Deserialize)]
struct ChromiumManifest {
    artifacts: Vec<ChromiumArtifact>,
}
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct ChromiumArtifact {
    platform: String,
    executable: String,
}

fn manifest_artifact() -> Result<ChromiumArtifact, String> {
    let manifest: ChromiumManifest = serde_json::from_str(include_str!(
        "../../vendor/chromium/150.0.7871.115/manifest.json"
    ))
    .map_err(invalid_manifest)?;
    manifest_artifact_for_platform(manifest.artifacts, platform_key())
}

fn manifest_artifact_for_platform(
    artifacts: Vec<ChromiumArtifact>,
    platform: &str,
) -> Result<ChromiumArtifact, String> {
    artifacts
        .into_iter()
        .find(|artifact| artifact.platform == platform)
        .ok_or(missing_platform(platform))
}

fn missing_platform(platform: &str) -> String {
    format!("KRR Chromium manifest has no artifact for {platform}")
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn platform_key() -> &'static str {
    "mac-arm64"
}
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn platform_key() -> &'static str {
    "mac-x64"
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn platform_key() -> &'static str {
    "linux64"
}
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn platform_key() -> &'static str {
    "win64"
}
#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
fn platform_key() -> &'static str {
    "unsupported"
}

fn executable_file(path: PathBuf, source: &str) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "{source} executable was not found at {}",
            path.display()
        ))
    }
}

fn invalid_manifest(error: serde_json::Error) -> String {
    format!("invalid KRR Chromium manifest: {error}")
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_selects_the_current_platform_artifact() {
        let artifact = must(manifest_artifact());

        assert_eq!(artifact.platform, platform_key());
        assert!(!artifact.executable.is_empty());
    }

    #[test]
    fn manifest_selection_reports_missing_platform() {
        assert_eq!(
            manifest_artifact_for_platform(
                vec![ChromiumArtifact {
                    platform: "other".to_string(),
                    executable: "chrome".to_string(),
                }],
                "missing"
            ),
            Err("KRR Chromium manifest has no artifact for missing".to_string())
        );
    }

    #[test]
    fn rendering_args_keep_browser_frames_available() {
        let args = rendering_args();

        assert_eq!(
            args,
            [
                OsStr::new("--disable-background-timer-throttling"),
                OsStr::new("--disable-backgrounding-occluded-windows"),
                OsStr::new("--disable-renderer-backgrounding"),
            ]
        );
    }

    #[test]
    fn runtime_error_helpers_preserve_contract_messages() {
        let manifest_error =
            serde_json::from_str::<ChromiumManifest>("{").map_err(invalid_manifest);

        assert!(
            matches!(manifest_error, Err(message) if message.starts_with("invalid KRR Chromium manifest: "))
        );
        assert_eq!(
            missing_platform("test-platform"),
            "KRR Chromium manifest has no artifact for test-platform"
        );
        assert_eq!(
            string_error(std::io::Error::other("runtime failed")),
            "runtime failed"
        );
    }

    #[test]
    fn executable_file_reports_missing_and_existing_paths() {
        let missing = std::env::temp_dir().join("krr-missing-chromium-test-binary");
        assert!(executable_file(missing, "test source").is_err());

        let existing = std::env::temp_dir().join(format!(
            "krr-existing-chromium-test-binary-{}",
            std::process::id()
        ));
        must(std::fs::write(&existing, b"test"));
        let resolved = must(executable_file(existing.clone(), "test source"));
        let _ = std::fs::remove_file(&existing);

        assert_eq!(resolved, existing);
    }

    #[test]
    fn chrome_binary_path_honors_explicit_environment_override() {
        let _guard = must(CHROMIUM_BINARY_ENV_LOCK.lock());
        let existing = std::env::temp_dir().join(format!(
            "krr-env-chromium-test-binary-{}",
            std::process::id()
        ));
        must(std::fs::write(&existing, b"test"));
        unsafe { std::env::set_var(CHROMIUM_BINARY_ENV, &existing) };
        let result = chrome_binary_path();
        unsafe { std::env::remove_var(CHROMIUM_BINARY_ENV) };
        let resolved = must(result);
        let _ = std::fs::remove_file(&existing);

        assert_eq!(resolved, existing);
    }

    #[test]
    fn chrome_binary_path_checks_packaged_bundle_when_no_override_is_set() {
        let _guard = must(CHROMIUM_BINARY_ENV_LOCK.lock());
        unsafe { std::env::remove_var(CHROMIUM_BINARY_ENV) };

        let _ = chrome_binary_path();
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom"));
    }

    #[test]
    fn must_error_branch_covers_lock_guard_type() {
        let lock = std::sync::Mutex::new(());
        let guard = must(lock.lock());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard_from_error: std::sync::MutexGuard<'_, ()> =
                    must::<
                        std::sync::MutexGuard<'_, ()>,
                        std::sync::PoisonError<std::sync::MutexGuard<'_, ()>>,
                    >(Err(std::sync::PoisonError::new(guard)));
            }))
            .is_err()
        );
    }

    #[test]
    fn must_error_branch_covers_runtime_value_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: ChromiumArtifact = must::<ChromiumArtifact, String>(Err("boom".to_string()));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: PathBuf = must::<PathBuf, String>(Err("boom".to_string()));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: () = must::<(), std::io::Error>(Err(std::io::Error::other("boom")));
            })
            .is_err()
        );
    }

    fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }
}
