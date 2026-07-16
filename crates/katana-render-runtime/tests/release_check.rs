use std::path::{Path, PathBuf};
use std::process::Command;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn release_check_requires_all_quality_and_publish_readiness_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = fs::read_to_string(root.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-check")?;

    for required_gate in [
        "release-openspec-archive",
        "check",
        "coverage",
        "release-verify",
    ] {
        assert!(
            recipe.contains(required_gate),
            "release-check must require {required_gate}"
        );
    }
    Ok(())
}

#[test]
fn release_target_check_allows_exactly_one_semver_step() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    for version in ["0.3.9", "0.4.0", "1.0.0"] {
        assert!(release_target_check(root, version)?);
    }
    for version in ["0.3.10", "0.4.1", "0.5.0", "2.0.0"] {
        assert!(!release_target_check(root, version)?);
    }
    Ok(())
}

#[test]
fn release_openspec_archive_allows_current_change_and_rejects_prior_change()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let fixture = archive_fixture_directory()?;
    let changes = fixture.join("openspec/changes");
    fs::create_dir_all(changes.join("v0-4-0-current-release"))?;

    let current = archive_check(root, &fixture)?;
    assert_archive_success(&current);

    fs::create_dir_all(changes.join("v0-3-9-prior-release"))?;
    let prior = archive_check(root, &fixture)?;
    fs::remove_dir_all(&fixture)?;

    assert!(!prior.status.success());
    assert!(String::from_utf8_lossy(&prior.stderr).contains("v0-3-9-prior-release"));
    Ok(())
}

#[test]
fn coverage_gate_includes_integration_test_targets() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = fs::read_to_string(root.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "coverage")?;

    assert!(recipe.contains("--all-targets"));
    Ok(())
}

#[test]
fn test_gates_install_required_runtimes_before_executing_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = fs::read_to_string(root.join("Justfile"))?;
    for recipe_name in ["unit-test", "coverage"] {
        let recipe = recipe_body(&justfile, recipe_name)?;
        assert!(
            recipe.trim_start().starts_with("plantuml-install"),
            "{recipe_name} must prepare the PlantUML runtime before execution"
        );
    }

    let workflow = fs::read_to_string(root.join(".github/workflows/test-and-build.yml"))?;
    let install = workflow
        .find("name: Install PlantUML runtime")
        .ok_or("PlantUML install step is missing")?;
    let tests = workflow
        .find("name: Run tests")
        .ok_or("test step is missing")?;
    assert!(
        install < tests,
        "PlantUML must be installed before workspace tests"
    );
    let chromium_install = workflow
        .find("name: Install Chromium browser runtime")
        .ok_or("Chromium install step is missing")?;
    assert!(
        chromium_install < tests,
        "Chromium must be installed before workspace tests"
    );
    Ok(())
}

#[test]
fn release_preflight_installs_graphviz_before_release_check()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let workflow = fs::read_to_string(root.join(".github/workflows/release-preflight.yml"))?;
    let graphviz = workflow
        .find("name: Install Graphviz for PlantUML")
        .ok_or("Graphviz install step is missing")?;
    let release_check = workflow
        .find("name: Release check")
        .ok_or("release check step is missing")?;

    assert!(
        graphviz < release_check,
        "Graphviz must be installed before release-check runs PlantUML tests"
    );
    Ok(())
}

#[test]
fn ci_browser_tests_are_serialized_and_timeout_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = fs::read_to_string(root.join("Justfile"))?;
    let unit_test = recipe_body(&justfile, "unit-test")?;
    let coverage = recipe_body(&justfile, "coverage")?;
    let ci_workflow = fs::read_to_string(root.join(".github/workflows/test-and-build.yml"))?;
    let preflight_workflow =
        fs::read_to_string(root.join(".github/workflows/release-preflight.yml"))?;

    assert_test_recipes_are_serialized(unit_test, coverage);
    assert_ci_workflow_bounds_browser_tests(&ci_workflow);
    assert_preflight_workflow_bounds_browser_tests(&preflight_workflow);
    Ok(())
}

fn assert_test_recipes_are_serialized(unit_test: &str, coverage: &str) {
    assert!(unit_test.contains("{{TEST_THREAD_ARGS}}"));
    assert!(unit_test.contains("--locked"));
    assert!(coverage.contains("{{TEST_THREAD_ARGS}}"));
}

fn assert_ci_workflow_bounds_browser_tests(ci_workflow: &str) {
    assert!(ci_workflow.contains("TEST_THREADS: \"1\""));
    assert!(ci_workflow.contains("timeout-minutes: 45"));
    assert!(ci_workflow.contains("run: just unit-test"));
    assert!(
        ci_workflow
            .contains("name: Cache Rust build outputs\n        if: matrix.os != 'ubuntu-latest'")
    );
    assert!(ci_workflow.contains("name: Free Ubuntu build space before coverage"));
    assert!(ci_workflow.contains("name: Free Ubuntu build space after coverage"));
    assert!(ci_workflow.contains("cargo llvm-cov clean --workspace"));
    assert!(!ci_workflow.contains("continue-on-error: true"));
}

fn assert_preflight_workflow_bounds_browser_tests(preflight_workflow: &str) {
    assert!(preflight_workflow.contains("TEST_THREADS: \"1\""));
    assert!(preflight_workflow.contains("timeout-minutes: 75"));
}

#[test]
fn release_verify_installs_and_checks_chromium_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = fs::read_to_string(root.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-verify")?;

    assert!(recipe.contains("chromium-install"));
    assert!(recipe.contains("chromium-asset-check"));
    Ok(())
}

#[test]
fn html_interactive_runtime_excludes_static_dom_bridge_api()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let public_api = fs::read_to_string(root.join("crates/katana-render-runtime/src/lib.rs"))?;
    let renderer_exports =
        fs::read_to_string(root.join("crates/katana-render-runtime/src/renderer/mod.rs"))?;
    let backend_exports =
        fs::read_to_string(root.join("crates/katana-render-runtime/src/renderer/backends/mod.rs"))?;
    let browser_runtime = fs::read_to_string(
        root.join("crates/katana-render-runtime/src/renderer/backends/html_browser/runtime.rs"),
    )?;
    let static_runtime = fs::read_to_string(
        root.join("crates/katana-render-runtime/src/renderer/backends/html.rs"),
    )?;

    for exposed_surface in [public_api.as_str(), renderer_exports.as_str()] {
        for forbidden in [
            "HtmlRuntimeDispatch",
            "HtmlRuntimeEvent",
            "HtmlNodeId",
            "HtmlNavigationIntent",
        ] {
            assert!(
                !exposed_surface.contains(forbidden),
                "{forbidden} must stay out of the public interactive HTML runtime API"
            );
        }
    }
    assert!(backend_exports.contains("pub use html_runtime::HtmlRuntimeError"));
    assert!(browser_runtime.contains("pub type HtmlRuntimeSession = HtmlBrowserSession"));
    assert!(browser_runtime.contains("HtmlBrowserSession::start(source, viewport, config)"));
    assert!(static_runtime.contains("StaticHtmlRuntime"));
    Ok(())
}

fn workspace_root() -> Result<&'static Path, Box<dyn std::error::Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| "workspace root is unavailable".into())
}

fn recipe_body<'a>(justfile: &'a str, recipe: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    let header = format!("{recipe}:");
    let start = justfile
        .find(&header)
        .ok_or_else(|| format!("{recipe} recipe is missing"))?;
    let body = &justfile[start + header.len()..];
    Ok(body.split("\n\n").next().unwrap_or(body))
}

fn release_target_check(
    root: &Path,
    target_version: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("python3")
        .args([
            "scripts/release/verify-release-target.py",
            "--target-version",
            target_version,
            "--latest-version",
            "0.3.8",
        ])
        .current_dir(root)
        .output()?;
    Ok(output.status.success())
}

fn archive_fixture_directory() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    let fixture = std::env::temp_dir().join(format!(
        "krr-release-openspec-archive-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&fixture)?;
    Ok(fixture)
}

fn archive_check(
    root: &Path,
    fixture: &Path,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    Command::new(bash_executable())
        .arg(bash_path(
            &root.join("scripts/release/check-openspec-release-archive.sh"),
        ))
        .arg("v0.4.0")
        .current_dir(fixture)
        .output()
        .map_err(Into::into)
}

fn bash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(windows)]
fn bash_executable() -> PathBuf {
    existing_path_from_env("GIT_BASH")
        .or_else(|| git_bash_from_program_files("ProgramFiles"))
        .or_else(|| git_bash_from_program_files("ProgramFiles(x86)"))
        .unwrap_or_else(|| PathBuf::from("bash"))
}

#[cfg(not(windows))]
fn bash_executable() -> PathBuf {
    PathBuf::from("bash")
}

#[cfg(windows)]
fn git_bash_from_program_files(name: &str) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os(name)?);
    let bash = root.join("Git").join("bin").join("bash.exe");
    bash.exists().then_some(bash)
}

#[cfg(windows)]
fn existing_path_from_env(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(name)?);
    path.exists().then_some(path)
}

fn assert_archive_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "archive check failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
