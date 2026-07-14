use super::{document::browser_document, source::BrowserSource};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use headless_chrome::protocol::cdp::Fetch;

const HTTP_OK_STATUS: u32 = 200;

pub(super) struct MainDocument {
    url: String,
    body: String,
}

impl MainDocument {
    pub(super) fn from_source(source: &BrowserSource) -> Option<Self> {
        matches!(source.origin_url.scheme(), "http" | "https").then(|| Self {
            url: source.origin_url.as_str().to_owned(),
            body: browser_document(source),
        })
    }

    pub(super) fn matches(&self, request_url: &str) -> bool {
        request_url == self.url
    }

    pub(super) fn fulfill(&self, request_id: Fetch::RequestId) -> Fetch::FulfillRequest {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_document_fulfills_http_origin_with_browser_document()
    -> Result<(), Box<dyn std::error::Error>> {
        let source =
            crate::HtmlBrowserSource::new("<p>remote</p>", "https://example.test/remote.html")?;
        let source = BrowserSource::validate(source)?;
        let document = MainDocument {
            url: source.origin_url.as_str().to_owned(),
            body: browser_document(&source),
        };

        assert!(MainDocument::from_source(&source).is_some());
        assert!(document.matches("https://example.test/remote.html"));
        assert!(!document.matches("https://example.test/other.html"));
        let fulfill = document.fulfill(Fetch::RequestId::from("request-1".to_string()));

        assert_eq!(fulfill.response_code, HTTP_OK_STATUS);
        assert!(fulfill.body.is_some());
        assert_ne!(fulfill.body, Some(String::new()));
        Ok(())
    }

    #[test]
    fn main_document_is_not_created_for_file_origin() -> Result<(), Box<dyn std::error::Error>> {
        let origin = url::Url::parse("file:///tmp/krr-main-document.html")?;
        let source = crate::HtmlBrowserSource::new("<p>local</p>", origin.to_string())?;
        let source = BrowserSource::validate(source)?;

        assert!(MainDocument::from_source(&source).is_none());
        Ok(())
    }
}
