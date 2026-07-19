use base64::Engine as _;
use percent_encoding::percent_decode_str;
use url::Url;

const MAX_SUBRESOURCE_BYTES: u64 = 8 * 1024 * 1024;

pub(super) fn load_text(url: &Url) -> Result<String, String> {
    String::from_utf8(load_bytes(url)?)
        .map_err(|error| format!("subresource is not UTF-8: {error}"))
}

pub(super) fn load_image_data_url(url: &Url) -> Result<String, String> {
    if url.scheme() == "data" {
        return Ok(url.to_string());
    }
    let bytes = load_bytes(url)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{};base64,{encoded}", image_mime_type(url)))
}

fn load_bytes(url: &Url) -> Result<Vec<u8>, String> {
    match url.scheme() {
        "data" => decode_data_url(url),
        "file" => std::fs::read(url.to_file_path().map_err(|_| "file URL is invalid")?)
            .map_err(|error| format!("local subresource could not be read: {error}")),
        "http" | "https" => load_http(url),
        scheme => Err(format!("unsupported subresource scheme: {scheme}")),
    }
}

fn load_http(url: &Url) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .build()
        .into();
    let mut response = agent
        .get(url.as_str())
        .call()
        .map_err(|error| format!("network subresource could not be read: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_SUBRESOURCE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("network subresource body could not be read: {error}"))
}

fn decode_data_url(url: &Url) -> Result<Vec<u8>, String> {
    let source = &url.as_str()["data:".len()..];
    let (metadata, payload) = source.split_once(',').ok_or("data URL has no payload")?;
    if metadata
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"))
    {
        return base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|error| format!("data URL base64 payload is invalid: {error}"));
    }
    Ok(percent_decode_str(payload).collect())
}

fn image_mime_type(url: &Url) -> &'static str {
    let extension = url.path().rsplit('.').next().map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

#[cfg(test)]
mod tests {
    use super::{image_mime_type, load_bytes, load_image_data_url, load_text};
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::Path;
    use std::thread;
    use url::Url;

    #[test]
    fn data_urls_support_percent_base64_and_image_passthrough() {
        with_url("data:text/plain,hello%20world", |percent| {
            assert_eq!(load_text(percent).ok().as_deref(), Some("hello world"));
        });
        with_url("data:text/plain;base64,aGVsbG8=", |base64| {
            assert_eq!(load_text(base64).ok().as_deref(), Some("hello"));
        });
        with_url("data:image/png;base64,AA==", |image| {
            assert_eq!(
                load_image_data_url(image).ok().as_deref(),
                Some(image.as_str())
            );
        });
        with_url("data:text/plain", |url| assert!(load_text(url).is_err()));
        with_url("data:text/plain;base64,!", |url| {
            assert!(load_text(url).is_err());
        });
        with_url("data:application/octet-stream;base64,/w==", |url| {
            assert!(load_text(url).is_err());
        });
    }

    #[test]
    fn unsupported_schemes_and_image_extensions_are_explicit() {
        with_url("ftp://example.test/image.png", |ftp| {
            assert!(load_bytes(ftp).is_err());
            assert_eq!(image_mime_type(ftp), "image/png");
        });
        assert_image_mime_type("image.gif", "image/gif");
        assert_image_mime_type("image.jpg", "image/jpeg");
        assert_image_mime_type("image.svg", "image/svg+xml");
        assert_image_mime_type("image.webp", "image/webp");
    }

    #[test]
    fn local_and_http_read_errors_are_reported() {
        with_url("file://example.test/unsupported", |url| {
            assert!(load_bytes(url).is_err());
        });
        with_file_url(&missing_file_path(), |url| {
            assert!(load_bytes(url).is_err());
            assert!(load_image_data_url(url).is_err());
        });
        with_url("http://127.0.0.1:0/unreachable", |url| {
            assert!(load_bytes(url).is_err());
        });
        with_truncated_http_response(|url| assert!(load_bytes(url).is_err()));
    }

    fn assert_image_mime_type(path: &str, expected: &str) {
        with_url(&format!("https://example.test/{path}"), |url| {
            assert_eq!(image_mime_type(url), expected);
        });
    }

    fn with_url(source: &str, assertion: impl FnMut(&Url)) {
        let parsed = Url::parse(source);
        assert!(parsed.is_ok(), "fixture URL must be valid: {source}");
        parsed.iter().for_each(assertion);
    }

    fn with_file_url(path: &Path, assertion: impl FnMut(&Url)) {
        let parsed = Url::from_file_path(path);
        assert!(parsed.is_ok(), "fixture file URL must be valid: {path:?}");
        parsed.iter().for_each(assertion);
    }

    fn missing_file_path() -> std::path::PathBuf {
        std::env::temp_dir().join("krr-html-subresource-missing-file")
    }

    fn with_truncated_http_response(mut assertion: impl FnMut(&Url)) {
        let listener = TcpListener::bind("127.0.0.1:0");
        assert!(listener.is_ok());
        listener.into_iter().for_each(|listener| {
            let address = listener.local_addr();
            assert!(address.is_ok());
            address.into_iter().for_each(|address| {
                thread::scope(|scope| {
                    scope.spawn(|| {
                        let accepted = listener.accept();
                        assert!(accepted.is_ok());
                        accepted.into_iter().for_each(|(mut stream, _)| {
                            assert!(
                                stream
                                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nx")
                                    .is_ok()
                            );
                        });
                    });
                    let url = Url::parse(&format!("http://{address}/truncated"));
                    assert!(url.is_ok());
                    url.iter().for_each(&mut assertion);
                });
            });
        });
    }
}
