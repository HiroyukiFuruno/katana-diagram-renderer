use super::archive::{resource_contents, validate_resource_archive};
use super::selector::{drawio_prefix, extract_resource_groups, resource_groups};
use super::{DrawioResourceCatalog, encoding_for_path, mime_type_for_path};

#[test]
fn builtin_selects_basic_stencil_shape_scripts_and_referenced_images() -> Result<(), String> {
    let source = r#"
        <mxCell style="shape=mxgraph.ios7ui.button;image=img/lib/azure2/general/File.svg"/>
    "#;
    let resources = DrawioResourceCatalog::builtin(source)?;

    assert!(resources.iter().any(|it| it.path == "stencils/basic.xml"));
    assert!(
        resources
            .iter()
            .any(|it| it.path == "stencils/ios7/misc.xml")
    );
    assert!(resources.iter().any(|it| it.path.starts_with("shapes/")));
    assert!(
        resources
            .iter()
            .any(|it| it.path == "img/lib/azure2/general/File.svg")
    );
    Ok(())
}

#[test]
fn resource_archive_validation_reports_length_and_bounds_errors() {
    assert!(validate_resource_archive(vec![1], 2).is_err());
    assert!(validate_resource_archive(vec![1], 1).is_ok());
    assert!(resource_contents(&[1], "overflow", usize::MAX, 2).is_err());
    assert!(resource_contents(&[1], "out-of-bounds", 0, 2).is_err());
    assert!(matches!(resource_contents(&[1], "valid", 0, 1), Ok([1])));
}

#[test]
fn mime_type_for_path_covers_supported_assets() {
    assert_eq!(mime_type_for_path("a.xml"), "text/xml");
    assert_eq!(mime_type_for_path("a.js"), "application/javascript");
    assert_eq!(mime_type_for_path("a.svg"), "image/svg+xml");
    assert_eq!(mime_type_for_path("a.png"), "image/png");
    assert_eq!(mime_type_for_path("a.jpg"), "image/jpeg");
    assert_eq!(mime_type_for_path("a.jpeg"), "image/jpeg");
    assert_eq!(mime_type_for_path("a.gif"), "image/gif");
    assert_eq!(mime_type_for_path("a.bin"), "application/octet-stream");
}

#[test]
fn encoding_for_path_covers_binary_and_text_assets() {
    assert!(matches!(
        encoding_for_path("a.png"),
        super::DrawioResourceEncoding::Base64
    ));
    assert_eq!(encoding_for_path("a.xml").as_str(), "text");
    assert_eq!(encoding_for_path("a.png").as_str(), "base64");
}

#[test]
fn resource_groups_and_prefix_handle_known_values() {
    assert_eq!(resource_groups("rackGeneral"), vec!["rack".to_string()]);
    assert_eq!(resource_groups("custom"), vec!["custom".to_string()]);
    assert!(extract_resource_groups("shape=mxgraph.rackGeneral.server").contains("rack"));
    assert_eq!(drawio_prefix(";"), None);
}
