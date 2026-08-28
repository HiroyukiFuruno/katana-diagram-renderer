use super::{
    DRAWIO_JS_CHECKSUM, DRAWIO_JS_VERSION, MERMAID_JS_CHECKSUM, MERMAID_JS_VERSION,
    MERMAID_ZENUML_JS_CHECKSUM, RuntimeAsset, ZENUML_CORE_JS_CHECKSUM, ZENUML_CORE_JS_VERSION,
};

const PARALLEL_MATERIALIZE_THREADS: usize = 8;

#[test]
fn materialized_paths_are_versioned() {
    let mermaid = RuntimeAsset::mermaid().materialized_path();
    let drawio = RuntimeAsset::drawio().materialized_path();
    let zenuml_core = RuntimeAsset::zenuml_core().materialized_path();

    assert!(mermaid.ends_with(format!(
        "vendor/mermaid/{MERMAID_JS_VERSION}/mermaid.min.js"
    )));
    assert!(drawio.ends_with(format!("vendor/drawio/{DRAWIO_JS_VERSION}/drawio.min.js")));
    assert!(zenuml_core.ends_with(format!(
        "vendor/zenuml-core/{ZENUML_CORE_JS_VERSION}/zenuml.js"
    )));
}

#[test]
fn pinned_checksums_are_sha256_hex() {
    assert_eq!(MERMAID_JS_CHECKSUM.len(), 64);
    assert_eq!(MERMAID_ZENUML_JS_CHECKSUM.len(), 64);
    assert_eq!(DRAWIO_JS_CHECKSUM.len(), 64);
    assert_eq!(ZENUML_CORE_JS_CHECKSUM.len(), 64);
    assert!(MERMAID_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
    assert!(
        MERMAID_ZENUML_JS_CHECKSUM
            .chars()
            .all(|it| it.is_ascii_hexdigit())
    );
    assert!(DRAWIO_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
    assert!(
        ZENUML_CORE_JS_CHECKSUM
            .chars()
            .all(|it| it.is_ascii_hexdigit())
    );
}

#[test]
fn materialize_writes_missing_asset_file() {
    let path = test_path("missing-mermaid.min.js");
    remove_parent(&path);

    let result = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(matches!(result, Ok(written) if written == path));
    assert!(path.exists());
    remove_parent(&path);
}

#[test]
fn materialize_reports_empty_path_and_read_errors() {
    let empty_path = RuntimeAsset::mermaid().materialize_at(std::path::PathBuf::new());
    assert!(matches!(empty_path, Err(error) if error.contains("parent")));

    let path = test_path("runtime-directory");
    let _ = std::fs::remove_dir_all(&path);
    let create_result = std::fs::create_dir_all(&path);
    assert!(create_result.is_ok());

    let read_error = RuntimeAsset::mermaid().materialize_at(path.clone());
    assert!(read_error.is_err());
    let _ = std::fs::remove_dir_all(&path);
    remove_parent(&path);
}

#[test]
fn materialize_replaces_different_existing_asset_file() {
    let path = test_path("stale-mermaid.min.js");
    remove_parent(&path);
    let parent = path.parent();
    assert!(matches!(parent, Some(it) if std::fs::create_dir_all(it).is_ok()));
    let write_result = std::fs::write(&path, b"stale");
    assert!(write_result.is_ok());

    let result = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(result.is_ok());
    let stored = std::fs::read(path.clone());
    assert!(matches!(stored, Ok(bytes) if bytes != b"stale"));
    remove_parent(&path);
}

#[test]
fn materialize_keeps_same_existing_asset_file() {
    let path = test_path("current-mermaid.min.js");
    remove_parent(&path);
    let first = RuntimeAsset::mermaid().materialize_at(path.clone());
    assert!(matches!(first, Ok(written) if written == path));

    let second = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(matches!(second, Ok(written) if written == path));
    remove_parent(&path);
}

#[test]
fn materialize_is_safe_for_parallel_callers() {
    let path = test_path("parallel-mermaid.min.js");
    remove_parent(&path);
    let handles = parallel_materialize_handles(&path);

    for handle in handles {
        let joined = handle.join();
        assert!(matches!(joined, Ok(Ok(written)) if written == path));
    }
    let stored = std::fs::read(path.clone());
    let asset = RuntimeAsset::mermaid();
    assert!(matches!(stored, Ok(bytes) if bytes.as_slice() == asset.bytes));
    remove_parent(&path);
}

#[test]
fn runtime_asset_error_keeps_io_error_message() {
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");

    let message = super::runtime_asset_error(error);

    assert_eq!(message, "denied");
}

#[test]
fn runtime_asset_cleanup_and_read_error_paths_are_explicit() {
    cleanup_temporary_file_after_rename_error();
    reports_read_error_for_directory_path();
}

fn cleanup_temporary_file_after_rename_error() {
    let temporary = test_path("cleanup.tmp");
    let parent = temporary.parent();
    assert!(matches!(parent, Some(it) if std::fs::create_dir_all(it).is_ok()));
    assert!(std::fs::write(&temporary, b"temporary").is_ok());
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "rename denied");

    let cleanup = RuntimeAsset::cleanup_temp_and_report(temporary.clone(), error);

    assert!(matches!(cleanup, Err(error) if error == "rename denied"));
    assert!(!temporary.exists());
}

fn reports_read_error_for_directory_path() {
    let directory = test_path("read-error");
    assert!(std::fs::create_dir_all(&directory).is_ok());
    let read_error = RuntimeAsset::mermaid().exists_with_same_bytes(&directory);

    assert!(read_error.is_err());
    remove_parent(&directory);
}

#[test]
fn runtime_asset_reports_rename_failure_after_temporary_write() {
    let destination = test_path("rename-destination");
    let parent = destination
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    assert!(std::fs::create_dir_all(parent).is_ok());
    assert!(std::fs::create_dir_all(&destination).is_ok());

    let result = RuntimeAsset::mermaid().write_atomically(&destination, parent);

    assert!(result.is_err());
    remove_parent(&destination);
}

#[test]
fn remove_parent_accepts_path_without_parent() {
    remove_parent(std::path::Path::new(""));
}

fn parallel_materialize_handles(
    path: &std::path::Path,
) -> Vec<std::thread::JoinHandle<Result<std::path::PathBuf, String>>> {
    (0..PARALLEL_MATERIALIZE_THREADS)
        .map(|_| {
            let thread_path = path.to_path_buf();
            std::thread::spawn(move || RuntimeAsset::mermaid().materialize_at(thread_path))
        })
        .collect()
}

fn test_path(name: &str) -> std::path::PathBuf {
    let slug = name.replace(['.', '/'], "-");
    std::env::temp_dir()
        .join(format!(
            "kdr-runtime-assets-test-{}-{slug}",
            std::process::id()
        ))
        .join(name)
}

fn remove_parent(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
