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
fn release_target_check_allows_only_v0_4_0() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    assert!(release_target_check(root, "0.4.0")?);
    for version in ["0.3.9", "0.3.10", "0.4.1", "0.5.0", "1.0.0", "2.0.0"] {
        assert!(!release_target_check(root, version)?);
    }
    assert!(!release_target_check_after(root, "0.4.0", "0.3.9")?);
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
    let clean = recipe
        .find("llvm-cov clean --workspace")
        .ok_or("coverage profile clean is missing")?;
    let collect = recipe
        .find("llvm-cov --workspace")
        .ok_or("coverage collection is missing")?;
    assert!(clean < collect);
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

    assert!(justfile.contains("TEST_THREADS := env_var_or_default(\"TEST_THREADS\", \"1\")"));
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
    assert_ci_runtime_asset_matrix(ci_workflow);
    assert!(!ci_workflow.contains("continue-on-error: true"));
}

fn assert_ci_runtime_asset_matrix(ci_workflow: &str) {
    for contract in [
        "os: ubuntu-latest\n            platform: linux64",
        "os: macos-15\n            platform: mac-arm64",
        "os: macos-15-intel\n            platform: mac-x64",
        "os: windows-latest\n            platform: win64",
        "name: Package Chromium release runtime",
        "just chromium-runtime-package",
    ] {
        assert!(ci_workflow.contains(contract), "CI is missing {contract}");
    }
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
    assert!(recipe.contains("chromium-runtime-package"));
    assert!(
        justfile.contains("--cache-dir \"{{CHROMIUM_CACHE_DIR}}\" --fresh"),
        "release runtime packaging must freshly extract the verified Chromium archive"
    );
    Ok(())
}

#[test]
fn release_workflow_publishes_every_chromium_runtime_without_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let workflow = fs::read_to_string(root.join(".github/workflows/release.yml"))?;
    let uploader = fs::read_to_string(root.join("scripts/release/upload-runtime-assets.sh"))?;

    assert_runtime_asset_matrix(&workflow);
    assert_runtime_release_order(&workflow)?;
    assert_immutable_runtime_uploader(&uploader);
    Ok(())
}

fn assert_runtime_asset_matrix(workflow: &str) {
    let immutable_checkout = "github.event.pull_request.merge_commit_sha || github.sha";
    for contract in [
        "os: ubuntu-latest\n            platform: linux64",
        "os: macos-15\n            platform: mac-arm64",
        "os: macos-15-intel\n            platform: mac-x64",
        "os: windows-latest\n            platform: win64",
        "python3 scripts/chromium/package_runtime.py",
        "--fresh",
        "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
        "needs: [release-context, runtime-assets]",
        "bash scripts/release/upload-runtime-assets.sh",
        immutable_checkout,
        "gh release create \"${TAG}\" --verify-tag --draft --generate-notes",
    ] {
        assert!(
            workflow.contains(contract),
            "release workflow is missing {contract}"
        );
    }
    assert_eq!(workflow.matches(immutable_checkout).count(), 3);
    assert!(!workflow.contains("github.event.pull_request.base.ref || github.ref"));
}

fn assert_runtime_release_order(workflow: &str) -> Result<(), Box<dyn std::error::Error>> {
    let package = workflow
        .find("name: Package and verify Chromium browser runtime")
        .ok_or("runtime package step is missing")?;
    let artifact = workflow
        .find("name: Upload verified runtime archive")
        .ok_or("runtime artifact step is missing")?;
    let release = workflow
        .find("name: Create GitHub Release")
        .ok_or("GitHub Release step is missing")?;
    let runtime_upload = workflow
        .find("name: Upload immutable Chromium runtime assets")
        .ok_or("runtime Release upload step is missing")?;
    let publish_release = workflow
        .find("name: Publish complete GitHub Release")
        .ok_or("complete GitHub Release publish step is missing")?;
    let crates = workflow
        .find("name: Publish crates.io")
        .ok_or("crates.io publish step is missing")?;
    assert!(package < artifact);
    assert!(release < runtime_upload);
    assert!(runtime_upload < publish_release);
    assert!(publish_release < crates);
    Ok(())
}

fn assert_immutable_runtime_uploader(uploader: &str) {
    assert!(uploader.contains("sha256sum --check"));
    assert!(uploader.contains("cmp -s"));
    assert!(!uploader.contains("--clobber"));
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

#[test]
fn html_interactive_runtime_preserves_raw_html_and_browser_navigation_semantics()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let engine = root.join("crates/katana-render-runtime/src/html_chromium_engine");
    let document = fs::read_to_string(engine.join("document.rs"))?;
    let input = fs::read_to_string(engine.join("input.rs"))?;
    let main_document = fs::read_to_string(engine.join("main_document.rs"))?;
    let navigation = fs::read_to_string(engine.join("navigation.rs"))?;
    let policy = fs::read_to_string(engine.join("policy.rs"))?;
    let popup_guard = fs::read_to_string(engine.join("popup_guard.rs"))?;

    assert_no_host_html_or_navigation_rewriting(&document, &navigation);
    assert_raw_main_document_policy(&main_document, &policy, &navigation);
    assert_popup_guard_contract(&input, &navigation, &popup_guard);
    Ok(())
}

fn assert_no_host_html_or_navigation_rewriting(document: &str, navigation: &str) {
    for forbidden in [
        "AddScriptToEvaluateOnNewDocument",
        "inject_head",
        "temporary_document",
        "closest('a[href]')",
        "__katanaTakeNavigation",
    ] {
        assert!(
            !document.contains(forbidden) && !navigation.contains(forbidden),
            "interactive HTML must not restore host document/navigation rewriting: {forbidden}"
        );
    }
}

fn assert_raw_main_document_policy(main_document: &str, policy: &str, navigation: &str) {
    assert!(main_document.contains("body: source.source.raw_html.clone()"));
    assert!(main_document.contains("self.pending.swap(false"));
    assert!(policy.contains("Network::ResourceType::Document"));
    assert!(policy.contains("navigation.is_root_frame"));
    assert!(navigation.contains("PageFrameRequestedNavigation"));
}

fn assert_popup_guard_contract(input: &str, navigation: &str, popup_guard: &str) {
    assert!(navigation.contains("PageWindowOpen"));
    assert!(!input.contains("get_tabs()"));
    assert!(popup_guard.contains("call_method_on_browser(Target::SetAutoAttach"));
    assert!(popup_guard.contains("wait_for_debugger_on_start: true"));
    assert!(popup_guard.contains("flatten: Some(true)"));
    assert!(popup_guard.contains("let close_target = Target::CloseTarget"));
    assert!(popup_guard.contains("call_method_on_browser(close_target)"));
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
    release_target_check_after(root, target_version, "0.3.8")
}

fn release_target_check_after(
    root: &Path,
    target_version: &str,
    latest_version: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("python3")
        .args([
            "scripts/release/verify-release-target.py",
            "--target-version",
            target_version,
            "--latest-version",
            latest_version,
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
