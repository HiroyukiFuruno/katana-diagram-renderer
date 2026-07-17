use super::source::BrowserSource;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use headless_chrome::protocol::cdp::Fetch;
use std::sync::atomic::{AtomicBool, Ordering};

const HTTP_OK_STATUS: u32 = 200;

pub(super) struct MainDocument {
    request_url: String,
    body: String,
    pending: AtomicBool,
}

impl MainDocument {
    pub(super) fn from_source(source: &BrowserSource) -> Option<Self> {
        matches!(source.origin_url.scheme(), "file" | "http" | "https").then(|| {
            let mut request_url = source.origin_url.clone();
            request_url.set_fragment(None);
            Self {
                request_url: request_url.into(),
                body: source.source.raw_html.clone(),
                pending: AtomicBool::new(true),
            }
        })
    }

    pub(super) fn matches(&self, request_url: &str) -> bool {
        request_url == self.request_url
    }

    pub(super) fn fulfill_once(
        &self,
        request_id: Fetch::RequestId,
        request_url: &str,
    ) -> Option<Fetch::FulfillRequest> {
        (self.matches(request_url) && self.pending.swap(false, Ordering::AcqRel)).then(|| {
            Fetch::FulfillRequest {
                request_id,
                response_code: HTTP_OK_STATUS,
                response_headers: Some(vec![Fetch::HeaderEntry {
                    name: "Content-Type".to_string(),
                    value: "text/html; charset=utf-8".to_string(),
                }]),
                binary_response_headers: None,
                body: Some(BASE64.encode(self.body.as_bytes())),
                response_phrase: Some("OK".to_string()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_document_fulfills_raw_html_at_the_source_origin()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw_html = "<p>remote</p>";
        let source =
            crate::HtmlBrowserSource::new(raw_html, "https://example.test/remote.html#section")?;
        let source = BrowserSource::validate(source)?;
        let document = MainDocument::from_source(&source).ok_or("main document was not created")?;

        assert!(document.matches("https://example.test/remote.html"));
        assert!(!document.matches("https://example.test/remote.html#section"));
        assert!(!document.matches("https://example.test/other.html"));
        let fulfill = document
            .fulfill_once(
                Fetch::RequestId::from("request-1".to_string()),
                "https://example.test/remote.html",
            )
            .ok_or("main document was not fulfilled")?;

        assert_eq!(fulfill.response_code, HTTP_OK_STATUS);
        assert_eq!(fulfill.body, Some(BASE64.encode(raw_html.as_bytes())));
        assert!(
            document
                .fulfill_once(
                    Fetch::RequestId::from("request-2".to_string()),
                    "https://example.test/remote.html",
                )
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn main_document_is_created_for_file_origin() -> Result<(), Box<dyn std::error::Error>> {
        let origin_path = std::env::temp_dir().join("krr-main-document.html");
        let origin = url::Url::from_file_path(origin_path)
            .map_err(|()| "temporary path is not a valid file URL")?;
        let source = crate::HtmlBrowserSource::new("<p>local</p>", origin.to_string())?;
        let source = BrowserSource::validate(source)?;

        assert!(MainDocument::from_source(&source).is_some());
        Ok(())
    }
}
