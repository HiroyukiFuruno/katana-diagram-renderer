use super::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlJarAssetOps,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn plantuml_asset_metadata_is_pinned() {
    assert_eq!(PLANTUML_JAR_CHECKSUM.len(), 64);
    assert!(PLANTUML_DOWNLOAD_URL.contains(PLANTUML_JAR_VERSION));
}

#[test]
fn cache_path_uses_explicit_root_and_pinned_version() {
    assert_eq!(
        PlantUmlJarAssetOps::cache_path(Some(Path::new("/tmp/krr-cache"))),
        Path::new("/tmp/krr-cache")
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    );
    assert_eq!(
        PlantUmlJarAssetOps::digest_bytes(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn cached_jar_must_match_pinned_checksum() -> Result<(), String> {
    let root = temp_root("cached-invalid");
    let path = PlantUmlJarAssetOps::cache_path(Some(&root));
    let parent = path.parent().ok_or("cache path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(&path, b"invalid jar").map_err(|error| error.to_string())?;

    let error = error_message(PlantUmlJarAssetOps::prepare_cache_jar(Some(&root)))?;

    assert!(error.contains("plantuml.jar checksum mismatch"));
    Ok(())
}

#[test]
fn rejects_missing_or_invalid_cache_targets_without_downloading() -> Result<(), String> {
    let missing_parent = download_to_invalid_cache_target(Path::new(""))?;
    let root = temp_root("invalid-parent");
    let blocker = root.join("blocker");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    std::fs::write(&blocker, b"not a directory").map_err(|error| error.to_string())?;
    let invalid_parent = download_to_invalid_cache_target(&blocker.join("plantuml.jar"))?;

    assert!(missing_parent.contains("has no parent"));
    assert!(invalid_parent.contains("cache directory is not writable"));
    Ok(())
}

#[test]
fn install_temp_file_handles_success_and_existing_invalid_destination() -> Result<(), String> {
    let root = temp_root("install");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    install_temp_file_success(&root)?;
    let error = install_temp_file_race_error(&root)?;

    assert!(error.contains("cache install raced and existing file is invalid"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn install_temp_file_reports_nonexistent_destination_error() -> Result<(), String> {
    let root = temp_root("install-error");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let temporary = root.join("temporary.jar");
    std::fs::write(&temporary, b"temporary").map_err(|error| error.to_string())?;
    set_mode(&root, 0o555)?;

    let result = PlantUmlJarAssetOps::install_temp_file(&temporary, &root.join("missing.jar"));

    set_mode(&root, 0o755)?;
    let error = error_message(result)?;
    assert!(error.contains("cache file could not be installed"));
    Ok(())
}

#[test]
fn cache_preparation_uses_verified_local_download_and_reuses_cache() -> Result<(), String> {
    let root = temp_root("prepared-cache");
    let path = root.join("plantuml.jar");
    let mut first_download = || Ok(b"fixture".to_vec());
    let mut second_download = || Err("cache should be reused".to_string());
    let first = PlantUmlJarAssetOps::prepare_cache_path(
        path.clone(),
        verify_fixture_bytes,
        &mut first_download,
    )?;
    let second = PlantUmlJarAssetOps::prepare_cache_path(
        path.clone(),
        verify_fixture_bytes,
        &mut second_download,
    )?;

    assert_eq!(first, path);
    assert_eq!(second, path);
    assert_eq!(
        std::fs::read(path).map_err(|error| error.to_string())?,
        b"fixture"
    );
    Ok(())
}

#[test]
fn cache_preparation_reports_existing_path_read_and_download_verify_errors() -> Result<(), String> {
    let directory = temp_root("existing-directory");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut unused_download = || Err("download should not run".to_string());
    let mut invalid_download = || Ok(b"not fixture".to_vec());
    let mut unavailable_download = || Err("download unavailable".to_string());
    let read_error = error_message(PlantUmlJarAssetOps::prepare_cache_path(
        directory,
        verify_fixture_bytes,
        &mut unused_download,
    ))?;
    let verify_error = error_message(PlantUmlJarAssetOps::download_to_cache_with(
        &temp_root("verify-error").join("plantuml.jar"),
        verify_fixture_bytes,
        &mut invalid_download,
    ))?;
    let download_error = error_message(PlantUmlJarAssetOps::download_to_cache_with(
        &temp_root("download-error").join("plantuml.jar"),
        verify_fixture_bytes,
        &mut unavailable_download,
    ))?;

    assert!(!read_error.is_empty());
    assert_eq!(verify_error, "unexpected fixture bytes");
    assert_eq!(download_error, "download unavailable");
    Ok(())
}

#[test]
fn download_to_cache_reports_temporary_file_write_errors() -> Result<(), String> {
    let root = temp_root("temporary-write-error");
    let path = root.join("plantuml.jar");
    let temporary = PlantUmlJarAssetOps::temp_path(&path);
    std::fs::create_dir_all(&temporary).map_err(|error| error.to_string())?;
    let mut download = || Ok(b"fixture".to_vec());

    let error = error_message(PlantUmlJarAssetOps::download_to_cache_with(
        &path,
        verify_fixture_bytes,
        &mut download,
    ))?;

    assert!(error.contains("PlantUML cache file is not writable"));
    Ok(())
}

fn install_temp_file_success(root: &Path) -> Result<(), String> {
    let temporary = root.join("success.tmp");
    let destination = root.join("success.jar");
    std::fs::write(&temporary, b"installed").map_err(|error| error.to_string())?;

    PlantUmlJarAssetOps::install_temp_file(&temporary, &destination)?;

    assert_eq!(
        std::fs::read(&destination).map_err(|error| error.to_string())?,
        b"installed"
    );
    Ok(())
}

fn install_temp_file_race_error(root: &Path) -> Result<String, String> {
    let temporary = root.join("race.tmp");
    let destination = root.join("race.jar");
    std::fs::write(&temporary, b"temporary").map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    error_message(PlantUmlJarAssetOps::install_temp_file(
        &temporary,
        &destination,
    ))
}

fn error_message<T>(result: Result<T, String>) -> Result<String, String> {
    match result {
        Ok(_) => Err("expected operation to fail".to_string()),
        Err(error) => Ok(error),
    }
}

fn verify_fixture_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes == b"fixture" {
        return Ok(());
    }
    Err("unexpected fixture bytes".to_string())
}

fn download_to_invalid_cache_target(path: &Path) -> Result<String, String> {
    let mut download = || Err("download should not run".to_string());
    error_message(PlantUmlJarAssetOps::download_to_cache_with(
        path,
        verify_fixture_bytes,
        &mut download,
    ))
}

fn temp_root(name: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "krr-plantuml-asset-{name}-{}-{id}",
        std::process::id()
    ))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}
