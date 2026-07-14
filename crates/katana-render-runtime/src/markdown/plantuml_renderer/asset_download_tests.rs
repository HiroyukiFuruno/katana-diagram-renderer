use super::PlantUmlJarAssetOps;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

type LocalJarServer = std::thread::JoinHandle<std::io::Result<()>>;

static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn local_download_path_reads_response_and_reports_connection_failure() -> Result<(), String> {
    let (url, server) = local_jar_server()?;
    let bytes = PlantUmlJarAssetOps::download_from_url(&url)?;
    let joined = server
        .join()
        .map_err(|_| "local server panicked".to_string())?;
    let error = PlantUmlJarAssetOps::download_from_url("http://127.0.0.1:9");

    assert_eq!(bytes, b"fixture");
    assert!(joined.is_ok());
    assert!(matches!(error, Err(message) if message.contains("PlantUML JAR download failed")));
    Ok(())
}

#[test]
fn local_download_path_reports_truncated_response_body() -> Result<(), String> {
    let (url, server) = truncated_jar_server()?;

    let error = error_message(PlantUmlJarAssetOps::download_from_url(&url))?;
    let joined = server
        .join()
        .map_err(|_| "truncated local server panicked".to_string())?;

    assert!(joined.is_ok());
    assert!(error.contains("PlantUML JAR download failed"));
    Ok(())
}

#[test]
fn first_download_rejects_a_response_that_does_not_match_the_pinned_jar() -> Result<(), String> {
    let (url, server) = local_jar_server()?;
    let root = temp_root("pinned-download");

    let error = error_message(PlantUmlJarAssetOps::prepare_cache_jar_from(
        Some(&root),
        &url,
    ))?;
    let joined = server
        .join()
        .map_err(|_| "local server panicked".to_string())?;

    assert!(joined.is_ok());
    assert!(error.contains("plantuml.jar checksum mismatch"));
    Ok(())
}

fn error_message<T>(result: Result<T, String>) -> Result<String, String> {
    match result {
        Ok(_) => Err("expected operation to fail".to_string()),
        Err(error) => Ok(error),
    }
}

fn local_jar_server() -> Result<(String, LocalJarServer), String> {
    local_server(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture")
}

fn truncated_jar_server() -> Result<(String, LocalJarServer), String> {
    local_server(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ncut")
}

fn local_server(response: &'static [u8]) -> Result<(String, LocalJarServer), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let server = std::thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        stream.write_all(response)
    });
    Ok((format!("http://{address}"), server))
}

fn temp_root(name: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "krr-plantuml-download-{name}-{}-{id}",
        std::process::id()
    ))
}
