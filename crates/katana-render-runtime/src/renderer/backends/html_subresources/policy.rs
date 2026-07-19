use super::super::html_browser::{HtmlBrowserOrigin, HtmlBrowserSource};
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Clone)]
pub(in crate::renderer::backends) struct HtmlSubresourcePolicy {
    origin: Url,
    local_root: Option<PathBuf>,
}

impl HtmlSubresourcePolicy {
    pub(in crate::renderer::backends) fn from_source(source: &HtmlBrowserSource) -> Self {
        let origin = source.origin.url().clone();
        let local_root = local_root(&origin);
        Self { origin, local_root }
    }

    pub(in crate::renderer::backends) fn resolve_subresource(
        &self,
        reference: &str,
    ) -> Result<Url, String> {
        let url = self.resolve(reference)?;
        self.allows_subresource(reference, &url)
            .then_some(url)
            .ok_or_else(|| format!("subresource is not allowed: {reference}"))
    }

    pub(in crate::renderer::backends) fn resolve_navigation(
        &self,
        reference: &str,
    ) -> Result<HtmlBrowserOrigin, String> {
        let url = self.resolve(reference)?;
        self.allows_navigation(reference, &url)
            .then(|| HtmlBrowserOrigin::from_validated_url(url))
            .ok_or_else(|| format!("navigation is not allowed: {reference}"))
    }

    fn resolve(&self, reference: &str) -> Result<Url, String> {
        let Ok(url) = Url::parse(reference) else {
            return self
                .origin
                .join(reference)
                .map_err(|error| format!("resource URL is invalid: {reference}: {error}"));
        };
        let scheme = url.scheme().to_string();
        matches!(scheme.as_str(), "data" | "file" | "http" | "https")
            .then_some(url)
            .ok_or_else(|| format!("unsupported resource scheme: {scheme}"))
    }

    fn allows_subresource(&self, reference: &str, url: &Url) -> bool {
        if url.scheme() == "data" {
            return true;
        }
        self.allows_same_origin(reference, url)
    }

    fn allows_navigation(&self, reference: &str, url: &Url) -> bool {
        if self.origin.scheme() == "file" {
            return relative_file_reference(reference) && self.allows_local_navigation(url);
        }
        url.scheme() != "data" && self.allows_same_origin(reference, url)
    }

    fn allows_same_origin(&self, reference: &str, url: &Url) -> bool {
        if self.origin.scheme() == "file" {
            return relative_file_reference(reference) && self.allows_local_file(url);
        }
        matches!(url.scheme(), "http" | "https") && url.origin() == self.origin.origin()
    }

    fn allows_local_file(&self, url: &Url) -> bool {
        self.local_root.as_ref().is_some_and(|root| {
            url.to_file_path()
                .ok()
                .and_then(|path| path.canonicalize().ok())
                .is_some_and(|path| path.starts_with(root))
        })
    }

    pub(in crate::renderer::backends) fn allows_local_navigation(&self, url: &Url) -> bool {
        self.local_root.as_ref().is_some_and(|root| {
            let Ok(path) = url.to_file_path() else {
                return false;
            };
            path.ancestors()
                .find(|candidate| candidate.exists())
                .and_then(|ancestor| ancestor.canonicalize().ok())
                .is_some_and(|ancestor| ancestor.starts_with(root))
        })
    }
}

fn local_root(origin: &Url) -> Option<PathBuf> {
    origin
        .to_file_path()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|path| path.canonicalize().ok())
}

fn relative_file_reference(reference: &str) -> bool {
    Url::parse(reference).is_err()
        && !Path::new(reference).is_absolute()
        && !reference.starts_with('/')
}
