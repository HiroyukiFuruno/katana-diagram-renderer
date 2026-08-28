pub(super) fn fake_bundle_with_cisco_placeholders() -> &'static str {
    FAKE_BUNDLE_WITH_CISCO_PLACEHOLDERS
}

const FAKE_BUNDLE_WITH_CISCO_PLACEHOLDERS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "10");
  svg.setAttribute("viewBox", "0 0 20 10");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cisco");
  const fillPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
  fillPath.setAttribute("fill", "fillcolor");
  fillPath.setAttribute("style", "fill: light-dark(fillcolor, #000000)");
  group.appendChild(fillPath);
  const secondaryFillStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryFillStroke.setAttribute("fill", "none");
  secondaryFillStroke.setAttribute("stroke", "fillcolor2");
  secondaryFillStroke.setAttribute("style", "stroke: light-dark(fillcolor2, #000000)");
  group.appendChild(secondaryFillStroke);
  const secondaryStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryStroke.setAttribute("fill", "none");
  secondaryStroke.setAttribute("stroke", "strokecolor2");
  group.appendChild(secondaryStroke);
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

pub(super) fn fake_bundle_with_unresolved_stencil_color() -> &'static str {
    FAKE_BUNDLE_WITH_UNRESOLVED_STENCIL_COLOR
}

const FAKE_BUNDLE_WITH_UNRESOLVED_STENCIL_COLOR: &str = r##"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "10");
  svg.setAttribute("viewBox", "0 0 20 10");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "salesforce");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("width", "20");
  rect.setAttribute("height", "10");
  rect.setAttribute("fill", "fillcolor2");
  rect.setAttribute("style", "fill: light-dark(fillcolor2, #000000)");
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
"##;
