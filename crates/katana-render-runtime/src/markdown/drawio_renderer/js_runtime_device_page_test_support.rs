const FAKE_BUNDLE_WITH_DEVICE_PAGE_CONTENT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "300px");
  svg.setAttribute("height", "200px");
  svg.setAttribute("viewBox", "0 0 300 200");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "100");
  rect.setAttribute("height", "60");
  group.appendChild(rect);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

pub(super) fn fake_bundle_with_device_page_content() -> &'static str {
    FAKE_BUNDLE_WITH_DEVICE_PAGE_CONTENT
}

pub(super) fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

pub(super) fn device_page_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=rect;" vertex="1" parent="1">
  <mxGeometry x="10" y="0" width="100" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

pub(super) fn device_page_with_source_top_padding() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" value="Label" style="shape=rect;strokeColor=none;whiteSpace=wrap;html=1;" vertex="1" parent="1">
  <mxGeometry x="10" y="10" width="100" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}
