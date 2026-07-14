use crate::{HtmlBrowserError, HtmlBrowserSource};
use url::Url;

#[derive(Clone)]
pub(super) struct BrowserSource {
    pub(super) source: HtmlBrowserSource,
    pub(super) origin_url: Url,
}

impl BrowserSource {
    pub(super) fn validate(source: HtmlBrowserSource) -> Result<Self, HtmlBrowserError> {
        let origin = source.origin.as_str().to_string();
        let origin_url = Url::parse(&origin).map_err(|_| HtmlBrowserError::InvalidOrigin {
            origin: origin.clone(),
        })?;
        let source = HtmlBrowserSource::new(source.raw_html, origin)?;
        Ok(Self { source, origin_url })
    }
}
