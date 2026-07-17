use super::source::BrowserSource;

pub(super) fn document_url(source: &BrowserSource) -> Result<String, String> {
    let origin = source.origin_url.clone();
    if matches!(origin.scheme(), "file" | "http" | "https") {
        return Ok(origin.into());
    }
    Err(format!(
        "unsupported browser document scheme: {}",
        origin.scheme()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HtmlBrowserSource;
    use url::Url;

    #[test]
    fn document_url_preserves_http_and_file_origins() -> Result<(), Box<dyn std::error::Error>> {
        for origin in [
            "https://example.test/doc.html?a=1&b=2".to_string(),
            Url::from_file_path(std::env::temp_dir().join("krr-document.html"))
                .map_err(|()| "temporary path is not a file URL")?
                .to_string(),
        ] {
            let source = browser_source("<!doctype html><p>ok</p>", &origin)?;

            assert_eq!(document_url(&source)?, origin);
        }
        Ok(())
    }

    #[test]
    fn document_url_reports_unsupported_validated_origin_defensively()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = BrowserSource {
            source: browser_source("<p>data</p>", "https://example.test/data.html")?.source,
            origin_url: Url::parse("data:text/html,ok")?,
        };

        assert_eq!(
            document_url(&source),
            Err("unsupported browser document scheme: data".to_string())
        );
        Ok(())
    }

    fn browser_source(
        raw_html: impl Into<String>,
        origin: impl Into<String>,
    ) -> Result<BrowserSource, crate::HtmlBrowserError> {
        BrowserSource::validate(HtmlBrowserSource::new(raw_html, origin)?)
    }
}
