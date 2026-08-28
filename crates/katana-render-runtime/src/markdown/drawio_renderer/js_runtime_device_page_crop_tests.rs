use super::DrawioJsRuntimeOps;
use super::device_page_test_support::{
    device_page_source, fake_bundle_with_device_page_content, temp_runtime_path,
};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_crops_device_page_from_rendered_content_bounds() {
    let path = temp_runtime_path("kdr-drawio-device-page-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_device_page_content()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(device_page_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="102px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"transform="translate(-10,0)""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_device_page_when_rotation_changes_rendered_bounds() {
    let path = temp_runtime_path("kdr-drawio-device-page-rotated-ellipse-unit");
    assert!(std::fs::write(&path, rotated_ellipse_bundle()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        rotated_ellipse_device_page_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"viewBox="0 0 300 200""#) && !svg.contains(r#"transform="translate("#)
        }),
        "{rendered:?}"
    );
}

fn rotated_ellipse_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"document.createElementNS("http://www.w3.org/2000/svg", "rect")"#,
            r#"document.createElementNS("http://www.w3.org/2000/svg", "ellipse")"#,
        )
        .replace(
            r#"rect.setAttribute("x", "0");"#,
            r#"rect.setAttribute("cx", "100");"#,
        )
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("cy", "100");"#,
        )
        .replace(
            r#"rect.setAttribute("width", "100");"#,
            r#"rect.setAttribute("rx", "80");"#,
        )
        .replace(
            r#"rect.setAttribute("height", "60");"#,
            r#"rect.setAttribute("ry", "20");
  rect.setAttribute("transform", "rotate(45,100,100)");"#,
        )
}

fn rotated_ellipse_device_page_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="ellipse;rotation=45;" vertex="1" parent="1">
  <mxGeometry x="20" y="80" width="160" height="40" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

#[test]
fn fake_bundle_uses_cropped_device_page_runtime_geometry_without_fractional_correction() {
    let path = temp_runtime_path("kdr-drawio-device-page-fractional-origin-unit");
    assert!(std::fs::write(&path, fractional_device_page_bundle()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(device_page_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"viewBox="0 0 102 62""#)),
        "{rendered:?}"
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"<rect x="0.31" y="0.31""#)
                && !svg.contains(r#"transform="translate(0,0)""#)
                && !svg.contains("translate(-0.31,-0.31)")
        }),
        "{rendered:?}"
    );
}

fn fractional_device_page_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"rect.setAttribute("x", "0");"#,
            r#"rect.setAttribute("x", "0.31");"#,
        )
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "0.31");"#,
        )
}

#[test]
fn fake_bundle_keeps_native_device_page_origin_when_content_crop_is_not_needed() {
    let path = temp_runtime_path("kdr-drawio-device-page-native-origin-unit");
    assert!(std::fs::write(&path, native_device_page_bundle()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(device_page_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"viewBox="0 0 102 62""#)),
        "{rendered:?}"
    );
}

fn native_device_page_bundle() -> String {
    fake_bundle_with_device_page_content()
        .replace(
            r#"svg.setAttribute("width", "300px");"#,
            r#"svg.setAttribute("width", "102px");"#,
        )
        .replace(
            r#"svg.setAttribute("height", "200px");"#,
            r#"svg.setAttribute("height", "62px");"#,
        )
        .replace(
            r#"svg.setAttribute("viewBox", "0 0 300 200");"#,
            r#"svg.setAttribute("viewBox", "0 0 102 62");"#,
        )
        .replace(
            r#"rect.setAttribute("x", "0");"#,
            r#"rect.setAttribute("x", "0.31");"#,
        )
        .replace(
            r#"rect.setAttribute("y", "0");"#,
            r#"rect.setAttribute("y", "0.31");"#,
        )
}
