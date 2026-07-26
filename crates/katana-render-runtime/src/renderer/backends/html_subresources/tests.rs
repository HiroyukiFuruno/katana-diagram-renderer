use super::{HtmlSubresourceLoader, HtmlSubresourcePolicy};
use crate::renderer::backends::{HtmlBrowserSource, HtmlRuntime};
use url::Url;

mod iframe;
#[path = "test_support.rs"]
mod support;
use support::{
    LocalFixture, TestResult, assert_frame_contains, local_resource_server, to_string, viewport,
};

#[test]
fn local_resources_feed_css_v8_and_image_layout() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = HtmlBrowserSource::new(fixture.html(), fixture.origin()?).map_err(to_string)?;
    let loader = HtmlSubresourceLoader::new(&source);
    assert_eq!(loader.document_origin(), source.origin.as_str());
    let mut document =
        crate::renderer::backends::html_document::HtmlDocument::parse(&source.raw_html);
    let resources = loader.load(&mut document).map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert!(resources.stylesheets.contains_key("style.css"));
    assert!(
        resources
            .scripts
            .iter()
            .any(|script| script.contains("scripted"))
    );
    assert!(document.render().contains("data:image/svg+xml;base64,"));
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    assert_frame_contains(&frame.pixels, [49, 130, 206]);
    Ok(())
}

#[test]
fn loopback_http_relative_css_script_and_image_requests_reach_the_runtime() -> TestResult {
    let (origin, requests, server) = local_resource_server()?;
    let source = HtmlBrowserSource::new(http_document(), format!("{origin}/index.html"))
        .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;
    let joined = server
        .join()
        .map_err(|_| "resource server panicked".to_string())?;

    assert!(joined.is_ok());
    assert_eq!(
        *requests
            .lock()
            .map_err(|_| "request log lock was poisoned".to_string())?,
        ["/style.css", "/app.js", "/pixel.svg"]
    );
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    assert_frame_contains(&frame.pixels, [49, 130, 206]);
    Ok(())
}

#[test]
fn policy_allows_network_subresources_and_rejects_local_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);

    assert_direct_reference_policy(&policy);
    assert_navigation_policy(&policy);
    assert_rejects_non_local_file_navigation(&policy)?;
    assert_page_survives_blocked_resources(&fixture)?;
    Ok(())
}

#[test]
fn https_documents_reject_mixed_content_but_allow_cross_origin_https() -> TestResult {
    let source =
        HtmlBrowserSource::new("<p>ok</p>", "https://docs.example/page.html").map_err(to_string)?;
    let policy = HtmlSubresourcePolicy::from_source(&source);

    assert!(
        policy
            .resolve_subresource("https://cdn.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("http://cdn.example/style.css")
            .is_err()
    );
    assert!(
        policy
            .resolve_subresource("https://user@cdn.example/style.css")
            .is_err()
    );
    assert!(
        policy
            .resolve_subresource("https://user:pass@cdn.example/style.css")
            .is_err()
    );
    Ok(())
}

fn assert_rejects_non_local_file_navigation(policy: &HtmlSubresourcePolicy) -> TestResult {
    let url = Url::parse("file://example.test/outside.html").map_err(to_string)?;

    assert!(!policy.allows_local_navigation(&url));
    Ok(())
}

fn assert_direct_reference_policy(policy: &HtmlSubresourcePolicy) {
    assert!(policy.resolve_subresource("../outside.css").is_err());
    assert!(policy.resolve_subresource("/absolute.css").is_err());
    assert!(policy.resolve_subresource("http://[").is_err());
    assert!(
        policy
            .resolve_subresource("https://other.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("http://other.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("data:text/css,body%7B%7D")
            .is_ok()
    );
}

fn assert_page_survives_blocked_resources(fixture: &LocalFixture) -> TestResult {
    let source = fixture.source(
        "<link rel=stylesheet href=style.css>\
         <link rel=stylesheet href=../outside.css>\
         <script src=file:///outside.js></script>\
         <img src=ftp://example.test/image.png>\
         <iframe src=https://other.example/frame.html></iframe>\
         <div id=styled>Visible</div>",
    )?;
    let mut document =
        crate::renderer::backends::html_document::HtmlDocument::parse(&source.raw_html);
    let resources = HtmlSubresourceLoader::new(&source)
        .load(&mut document)
        .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert_eq!(frame.viewport.width, 320);
    assert_eq!(frame.viewport.height, 240);
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    assert!(resources.stylesheets.contains_key("style.css"));
    assert!(document.render().contains("Visible"));
    Ok(())
}

fn assert_navigation_policy(policy: &HtmlSubresourcePolicy) {
    assert!(
        policy
            .resolve_navigation("https://other.example/next.html")
            .is_err()
    );
    assert!(policy.resolve_navigation("next.html").is_ok());
    assert!(policy.resolve_navigation("guide/next.html").is_ok());
    assert!(policy.resolve_navigation("../outside.html").is_err());
}

#[cfg(unix)]
#[test]
fn policy_rejects_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture
        .root
        .parent()
        .ok_or("fixture parent is missing")?
        .join("outside.css");
    std::fs::write(&outside, "#target { color: red; }").map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape.css")).map_err(to_string)?;

    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);
    assert!(policy.resolve_subresource("escape.css").is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn policy_rejects_navigation_through_a_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture.root.with_extension("outside");
    std::fs::create_dir_all(&outside).map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape")).map_err(to_string)?;

    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);
    let rejected = policy.resolve_navigation("escape/next.html").is_err();
    std::fs::remove_dir_all(&outside).map_err(to_string)?;
    assert!(rejected);
    Ok(())
}

fn http_document() -> String {
    "<link rel=stylesheet href=style.css><div id=styled>Styled</div>\
         <div id=scripted style='width:80px;height:30px'>Scripted</div>\
         <script src=app.js></script>\
         <img src=pixel.svg style='width:40px;height:40px'>"
        .to_string()
}
