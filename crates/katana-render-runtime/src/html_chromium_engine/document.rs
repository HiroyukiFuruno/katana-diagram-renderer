use super::source::BrowserSource;
use crate::HtmlBrowserOrigin;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use url::Url;

static NEXT_TEMP_DOCUMENT: AtomicU64 = AtomicU64::new(0);

pub(super) fn document_url(source: &BrowserSource) -> Result<(String, Option<PathBuf>), String> {
    let origin = source.origin_url.clone();
    if matches!(origin.scheme(), "http" | "https") {
        return Ok((origin.into(), None));
    }
    let document = browser_document(source);
    if origin.scheme() == "file" {
        return local_document_url(origin, document);
    }
    Err(format!(
        "unsupported browser document scheme: {}",
        origin.scheme()
    ))
}

pub(super) fn browser_document(source: &BrowserSource) -> String {
    let head = format!(
        "<base href=\"{}\"><script>document.addEventListener('click', event => {{ const link = event.target.closest('a[href]'); if (link && !event.defaultPrevented) {{ event.preventDefault(); window.__katanaNavigation = new URL(link.href, document.baseURI).href; }} }})</script>",
        html_attribute(&source.source.origin),
    );
    inject_head(&source.source.raw_html, &head)
}

fn local_document_url(origin: Url, document: String) -> Result<(String, Option<PathBuf>), String> {
    if origin.host_str().is_some() {
        return Err(invalid_file_origin(()));
    }
    if origin.path() == "/" {
        return Err("file origin has no parent directory".to_string());
    }
    origin.to_file_path().map_err(invalid_file_origin)?;
    let suffix = NEXT_TEMP_DOCUMENT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        ".katana-krr-browser-{}-{suffix}.html",
        std::process::id()
    ));
    write_temporary_document(&path, &document)?;
    temporary_document_url(&path).map(|url| (url, Some(path)))
}

fn write_temporary_document(path: &std::path::Path, document: &str) -> Result<(), String> {
    fs::write(path, document).map_err(io_error)
}

fn temporary_document_url(path: &std::path::Path) -> Result<String, String> {
    match Url::from_file_path(path) {
        Ok(url) => Ok(url.into()),
        Err(()) => Err(temporary_document_url_error(())),
    }
}

fn inject_head(raw_html: &str, head: &str) -> String {
    let lower = raw_html.to_ascii_lowercase();
    let Some(head_start) = lower.find("<head") else {
        return format!("<!doctype html><html><head>{head}</head><body>{raw_html}</body></html>");
    };
    let Some(offset) = raw_html[head_start..].find('>') else {
        return format!("<!doctype html><html><head>{head}</head><body>{raw_html}</body></html>");
    };
    let insertion = head_start + offset + 1;
    format!(
        "{}{}{}",
        &raw_html[..insertion],
        head,
        &raw_html[insertion..]
    )
}

fn html_attribute(origin: &HtmlBrowserOrigin) -> String {
    origin
        .as_str()
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

fn invalid_file_origin(_: ()) -> String {
    "invalid file origin".to_string()
}

fn temporary_document_url_error(_: ()) -> String {
    "could not create temporary document URL".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HtmlBrowserSource;

    #[test]
    fn inject_head_inserts_into_existing_head() {
        let document = inject_head("<html><head><title>x</title></head></html>", "<base>");

        assert_eq!(document, "<html><head><base><title>x</title></head></html>");
    }

    #[test]
    fn inject_head_wraps_documents_without_a_complete_head_tag() {
        assert_eq!(
            inject_head("<p>body</p>", "<base>"),
            "<!doctype html><html><head><base></head><body><p>body</p></body></html>"
        );
        assert_eq!(
            inject_head("<html><head", "<base>"),
            "<!doctype html><html><head><base></head><body><html><head</body></html>"
        );
    }

    #[test]
    fn document_url_for_http_origin_uses_origin_url() {
        let raw_html = "<!doctype html><head></head><body>ok</body>";
        let origin = "https://example.test/doc.html?a=1&b=2";
        let source = browser_source(raw_html, origin);
        let (url, temporary_document) = must(document_url(&source));

        assert_eq!(url, origin);
        assert_eq!(temporary_document, None);
    }

    #[test]
    fn browser_document_injects_http_origin_base() {
        let raw_html = "<!doctype html><head></head><body>ok</body>";
        let origin = "https://example.test/doc.html?a=1&b=2";
        let source = browser_source(raw_html, origin);
        let document = browser_document(&source);

        assert!(document.contains("<base href=\"https://example.test/doc.html?a=1&amp;b=2\""));
        assert!(document.contains("!event.defaultPrevented"));
        assert!(document.contains("window.__katanaNavigation"));
    }

    #[test]
    fn document_url_for_file_origin_writes_temporary_document_outside_source_directory() {
        let directory =
            std::env::temp_dir().join(format!("krr-local-html-document-{}", std::process::id()));
        must(fs::create_dir_all(&directory));
        let origin_path = directory.join("index.html");
        must(fs::write(&origin_path, b"origin"));
        let origin = must_file_url(&origin_path);
        let source = browser_source("<p>local</p>", origin.to_string());

        let (url, temporary_document) = must(document_url(&source));
        let temporary_document =
            must(temporary_document.ok_or("temporary document was not written"));
        let body = must(fs::read_to_string(&temporary_document));
        let _ = fs::remove_file(&temporary_document);
        let _ = fs::remove_file(&origin_path);
        let _ = fs::remove_dir(&directory);

        assert!(url.starts_with("file://"));
        assert_eq!(
            temporary_document.parent(),
            Some(std::env::temp_dir().as_path())
        );
        assert!(body.contains("<base href=\"file://"));
        assert!(body.contains("<p>local</p>"));
    }

    #[test]
    fn document_url_reports_unsupported_validated_origin_defensively() {
        let source = BrowserSource {
            source: browser_source("<p>data</p>", "https://example.test/data.html").source,
            origin_url: must(Url::parse("data:text/html,ok")),
        };

        assert_eq!(
            document_url(&source),
            Err("unsupported browser document scheme: data".to_string())
        );
    }

    #[test]
    fn local_document_url_reports_invalid_file_origin() {
        let origin = must(Url::parse("file://host/path/index.html"));

        assert_eq!(
            local_document_url(origin, "<p>local</p>".to_string()),
            Err("invalid file origin".to_string())
        );
    }

    #[test]
    fn local_document_url_reports_missing_parent_directory() {
        let origin = must(Url::parse("file:///"));

        assert_eq!(
            local_document_url(origin, "<p>local</p>".to_string()),
            Err("file origin has no parent directory".to_string())
        );
    }

    #[test]
    fn temporary_document_write_reports_errors() {
        let directory = std::env::temp_dir().join(format!(
            "krr-temporary-document-write-error-{}",
            std::process::id()
        ));
        must(fs::create_dir_all(&directory));
        let result = write_temporary_document(&directory, "<p>local</p>");
        let _ = fs::remove_dir(&directory);

        assert!(result.is_err());
    }

    #[test]
    fn html_attribute_escapes_navigation_origin() {
        let origin = must(HtmlBrowserOrigin::parse(
            "https://example.test/path?a=1&b=2",
        ));

        assert_eq!(
            html_attribute(&origin),
            "https://example.test/path?a=1&amp;b=2"
        );
    }

    #[test]
    fn document_error_helpers_preserve_contract_messages() {
        assert_eq!(
            io_error(std::io::Error::other("document failed")),
            "document failed"
        );
        assert_eq!(invalid_file_origin(()), "invalid file origin");
        assert_eq!(
            temporary_document_url_error(()),
            "could not create temporary document URL"
        );
        assert_eq!(
            temporary_document_url(std::path::Path::new("relative.html")),
            Err("could not create temporary document URL".to_string())
        );
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom"));
    }

    #[test]
    #[should_panic(expected = "test path did not convert to file URL")]
    fn must_file_url_reports_invalid_paths() {
        let _ = must_file_url(std::path::Path::new("relative.html"));
    }

    #[test]
    fn must_error_branch_covers_url_and_source_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: Url = must::<Url, url::ParseError>(Err(url::ParseError::EmptyHost));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: BrowserSource = must::<BrowserSource, crate::HtmlBrowserError>(Err(
                    crate::HtmlBrowserError::InvalidViewport,
                ));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: HtmlBrowserSource = must::<HtmlBrowserSource, crate::HtmlBrowserError>(Err(
                    crate::HtmlBrowserError::InvalidViewport,
                ));
            })
            .is_err()
        );
    }

    #[test]
    fn must_error_branch_covers_path_and_origin_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: String = must::<String, std::io::Error>(Err(std::io::Error::other("boom")));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: std::path::PathBuf = must::<std::path::PathBuf, &str>(Err("boom"));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: crate::HtmlBrowserOrigin =
                    must::<crate::HtmlBrowserOrigin, crate::HtmlBrowserError>(Err(
                        crate::HtmlBrowserError::InvalidViewport,
                    ));
            })
            .is_err()
        );
    }

    #[test]
    fn must_error_branch_covers_document_result_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: (String, Option<std::path::PathBuf>) =
                    must::<(String, Option<std::path::PathBuf>), String>(Err("boom".to_string()));
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
            Err(error) => panic_message(format!("unexpected test error: {error}")),
        }
    }

    fn must_file_url(path: &std::path::Path) -> Url {
        match Url::from_file_path(path) {
            Ok(url) => url,
            Err(()) => panic_message("test path did not convert to file URL"),
        }
    }

    fn browser_source(raw_html: impl Into<String>, origin: impl Into<String>) -> BrowserSource {
        let source = must(HtmlBrowserSource::new(raw_html, origin));
        must(BrowserSource::validate(source))
    }

    fn panic_message(message: impl Into<String>) -> ! {
        std::panic::resume_unwind(Box::new(message.into()))
    }
}
