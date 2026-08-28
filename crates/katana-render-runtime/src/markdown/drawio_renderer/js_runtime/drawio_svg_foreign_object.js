const KATANA_DRAWIO_XHTML_NAMESPACE = "http://www.w3.org/1999/xhtml";

function katanaNormalizeDrawioForeignObjects(svg) {
  Array.from(svg.querySelectorAll("foreignObject")).forEach(katanaNormalizeDrawioForeignObject);
}

function katanaNormalizeDrawioForeignObject(foreignObject) {
  Array.from(foreignObject.children).forEach(katanaApplyDrawioXhtmlNamespace);
  if (katanaDrawioSourceStyleForElement(foreignObject).get("overflow") === "fill") {
    katanaNormalizeDrawioOverflowFillForeignObject(foreignObject);
  }
}

function katanaNormalizeDrawioOverflowFillForeignObject(foreignObject) {
  if (katanaDrawioIsHtmlClassDiagramSource()) {
    return;
  }
  const outer = foreignObject.children[0];
  if (!outer) {
    return;
  }
  const paddingTop =
    Number.parseFloat(katanaDrawioStylePropertyValue(outer.getAttribute("style"), "padding-top")) ||
    0;
  const centered = katanaDrawioStyleWithProperty(
    outer.getAttribute("style"),
    "align-items",
    "unsafe center",
  );
  outer.setAttribute(
    "style",
    katanaDrawioStyleWithProperty(centered, "padding-top", `${paddingTop + 1}px`),
  );
  const content = outer.children[0]?.children[0];
  if (!content) {
    return;
  }
  const wrapping = katanaDrawioStyleWithProperty(
    content.getAttribute("style"),
    "white-space",
    "normal",
  );
  content.setAttribute(
    "style",
    katanaDrawioStyleWithAddedProperty(wrapping, "word-wrap", "normal"),
  );
}

function katanaDrawioIsHtmlClassDiagramSource() {
  const source = katanaDrawioRequestSource();
  return [
    katanaDrawioSourceDisablesPageBounds(),
    (source.match(/&lt;hr \/&gt;/g) ?? []).length >= 3,
    (source.match(/margin-top: 4px/g) ?? []).length >= 3,
  ].every(Boolean);
}

function katanaApplyDrawioXhtmlNamespace(node) {
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return;
  }
  node.setAttribute("xmlns", KATANA_DRAWIO_XHTML_NAMESPACE);
}
