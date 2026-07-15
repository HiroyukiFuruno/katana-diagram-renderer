use super::{main_document::MainDocument, source::BrowserSource};
use headless_chrome::{
    browser::tab::RequestPausedDecision,
    protocol::cdp::{Fetch, Network},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use url::Url;

#[derive(Clone)]
pub(super) struct BrowserResourcePolicy {
    source_origin: Url,
    local_root: Option<PathBuf>,
    temporary_document: Option<PathBuf>,
}

impl BrowserResourcePolicy {
    pub(super) fn from_source(source: &BrowserSource) -> Self {
        let source_origin = source.origin_url.clone();
        let local_root = source_origin
            .to_file_path()
            .ok()
            .and_then(|path| path.parent().and_then(|parent| parent.canonicalize().ok()));
        Self {
            source_origin,
            local_root,
            temporary_document: None,
        }
    }

    pub(super) fn from_source_with_temporary_document(
        source: &BrowserSource,
        temporary_document: Option<&std::path::Path>,
    ) -> Self {
        let mut policy = Self::from_source(source);
        policy.temporary_document = temporary_document.map(std::path::Path::to_path_buf);
        policy
    }

    pub(super) fn allows(&self, request_url: &str) -> bool {
        let Ok(request_url) = Url::parse(request_url) else {
            return false;
        };
        match request_url.scheme() {
            "data" => true,
            "file" => {
                self.local_root.as_ref().is_some_and(|root| {
                    request_url
                        .to_file_path()
                        .ok()
                        .and_then(|path| path.canonicalize().ok())
                        .is_some_and(|path| path.starts_with(root))
                }) || request_url.to_file_path().ok().is_some_and(|path| {
                    self.temporary_document
                        .as_ref()
                        .is_some_and(|document| path == *document)
                })
            }
            "http" | "https" => {
                matches!(self.source_origin.scheme(), "http" | "https")
                    && request_url.origin() == self.source_origin.origin()
            }
            _ => false,
        }
    }
}

pub(super) fn install_resource_policy(
    tab: &Arc<headless_chrome::Tab>,
    source: &BrowserSource,
    temporary_document: Option<&Path>,
) -> Result<(), String> {
    let policy =
        BrowserResourcePolicy::from_source_with_temporary_document(source, temporary_document);
    let main_document = MainDocument::from_source(source);
    tab.enable_request_interception(Arc::new(
        move |_transport, _session_id, event: Fetch::events::RequestPausedEvent| {
            request_decision(event, &main_document, &policy)
        },
    ))
    .map_err(string_error)?;
    tab.enable_fetch(None, None)
        .map(|_| ())
        .map_err(string_error)
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

fn request_decision(
    event: Fetch::events::RequestPausedEvent,
    main_document: &Option<MainDocument>,
    policy: &BrowserResourcePolicy,
) -> RequestPausedDecision {
    if let Some(document) = main_document
        && document.matches(&event.params.request.url)
    {
        return RequestPausedDecision::Fulfill(document.fulfill(event.params.request_id));
    }
    if policy.allows(&event.params.request.url) {
        RequestPausedDecision::Continue(None)
    } else {
        RequestPausedDecision::Fail(Fetch::FailRequest {
            request_id: event.params.request_id,
            error_reason: Network::ErrorReason::BlockedByClient,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HtmlBrowserSource;

    #[test]
    fn http_sources_allow_data_and_same_origin_only() -> Result<(), String> {
        let source = browser_source("<p>ok</p>", "https://example.test/path/index.html")?;
        let policy = BrowserResourcePolicy::from_source(&source);

        assert!(policy.allows("data:text/css,body{}"));
        assert!(policy.allows("https://example.test/assets/style.css"));
        assert!(!policy.allows("https://other.test/assets/style.css"));
        assert!(!policy.allows("file:///tmp/krr-policy-local.css"));
        assert!(!policy.allows("not a url"));
        assert!(!policy.allows("ftp://example.test/file.css"));
        Ok(())
    }

    #[test]
    fn file_sources_allow_only_canonical_children() -> Result<(), String> {
        let directory =
            std::env::temp_dir().join(format!("krr-policy-local-root-{}", std::process::id()));
        let child = directory.join("child.css");
        let outside =
            std::env::temp_dir().join(format!("krr-policy-outside-{}.css", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(io_error)?;
        std::fs::write(&child, b"body{}").map_err(io_error)?;
        std::fs::write(&outside, b"body{}").map_err(io_error)?;
        let origin = must_file_url(&directory.join("index.html"))?;
        let child_url = must_file_url(&child)?;
        let outside_url = must_file_url(&outside)?;
        let source = browser_source("<p>ok</p>", origin.to_string())?;
        let policy = BrowserResourcePolicy::from_source(&source);

        let child_allowed = policy.allows(child_url.as_str());
        let outside_allowed = policy.allows(outside_url.as_str());
        let _ = std::fs::remove_file(&child);
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir(&directory);

        assert!(child_allowed);
        assert!(!outside_allowed);
        Ok(())
    }

    #[test]
    fn file_sources_allow_only_the_explicit_temporary_document_outside_the_local_root()
    -> Result<(), String> {
        let (directory, temporary_document, other_file) = temporary_document_fixture_paths();
        std::fs::create_dir_all(&directory).map_err(io_error)?;
        std::fs::write(&temporary_document, b"<!doctype html>").map_err(io_error)?;
        std::fs::write(&other_file, b"<!doctype html>").map_err(io_error)?;
        let origin = must_file_url(&directory.join("index.html"))?;
        let temporary_url = must_file_url(&temporary_document)?;
        let other_url = must_file_url(&other_file)?;
        let source = browser_source("<p>ok</p>", origin.to_string())?;
        let policy = BrowserResourcePolicy::from_source_with_temporary_document(
            &source,
            Some(&temporary_document),
        );

        let temporary_allowed = policy.allows(temporary_url.as_str());
        let other_allowed = policy.allows(other_url.as_str());
        let _ = std::fs::remove_file(&temporary_document);
        let _ = std::fs::remove_file(&other_file);
        let _ = std::fs::remove_dir(&directory);

        assert!(temporary_allowed);
        assert!(!other_allowed);
        Ok(())
    }

    fn temporary_document_fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "krr-policy-temporary-document-root-{}",
            std::process::id()
        ));
        let temporary_document = std::env::temp_dir().join(format!(
            "krr-policy-temporary-document-{}.html",
            std::process::id()
        ));
        let other_file = std::env::temp_dir().join(format!(
            "krr-policy-temporary-document-other-{}.html",
            std::process::id()
        ));
        (directory, temporary_document, other_file)
    }

    #[test]
    fn must_file_url_reports_invalid_paths() {
        assert_eq!(
            must_file_url(std::path::Path::new("relative.html")),
            Err("test path did not convert to file URL".to_string())
        );
    }

    #[test]
    fn test_error_helpers_preserve_messages() {
        assert_eq!(
            html_browser_error(crate::HtmlBrowserError::InvalidViewport),
            "browser viewport dimensions must be non-zero"
        );
        assert_eq!(io_error(std::io::Error::other("boom")), "boom");
        assert_eq!(string_error("policy failed"), "policy failed");
    }

    fn must_file_url(path: &std::path::Path) -> Result<Url, String> {
        match Url::from_file_path(path) {
            Ok(url) => Ok(url),
            Err(()) => Err("test path did not convert to file URL".to_string()),
        }
    }

    fn browser_source(
        raw_html: impl Into<String>,
        origin: impl Into<String>,
    ) -> Result<BrowserSource, String> {
        let source = HtmlBrowserSource::new(raw_html, origin).map_err(html_browser_error)?;
        BrowserSource::validate(source).map_err(html_browser_error)
    }

    fn html_browser_error(error: crate::HtmlBrowserError) -> String {
        error.to_string()
    }

    fn io_error(error: std::io::Error) -> String {
        error.to_string()
    }
}
