function katanaNormalizeDrawioGeometry(svg) {
  if (katanaDrawioNeedsNearIntegerGeometryRounding()) {
    katanaRoundDrawioNearIntegerPaths(svg);
  }
  if (!katanaDrawioRequestSource().includes("mxgraph.aws")) {
    return;
  }
  Array.from(svg.querySelectorAll("g"))
    .filter(katanaDrawioIsCellContentGroup)
    .filter(katanaDrawioNeedsHalfPixelTranslate)
    .forEach(katanaApplyDrawioHalfPixelTranslate);
}

function katanaDrawioIsCellContentGroup(group) {
  return group.parentNode?.getAttribute?.("data-cell-id");
}

function katanaDrawioNeedsHalfPixelTranslate(group) {
  return [
    !group.getAttribute("transform"),
    katanaDrawioGroupHasDirectShape(group),
    !katanaDrawioGroupBelongsToSourceEdge(group),
    katanaDrawioGroupUsesAwsCrispGeometry(group),
    katanaDrawioGroupUsesOddStrokeWidth(group),
  ].every(Boolean);
}

function katanaDrawioGroupUsesOddStrokeWidth(group) {
  const id = group.parentNode?.getAttribute?.("data-cell-id");
  const width = Number(KATANA_DRAWIO_SOURCE_CELL_STYLE_CACHE.get(id)?.get("strokeWidth") ?? 1);
  return Number.isInteger(width) && Math.abs(width % 2) === 1;
}

function katanaDrawioGroupUsesAwsCrispGeometry(group) {
  const id = group.parentNode?.getAttribute?.("data-cell-id");
  const shape = KATANA_DRAWIO_SOURCE_CELL_STYLE_CACHE.get(id)?.get("shape") ?? "";
  return shape === "" || shape === "mxgraph.aws4.resourceIcon";
}

function katanaDrawioGroupBelongsToSourceEdge(group) {
  const id = group.parentNode?.getAttribute?.("data-cell-id");
  return Array.from(katanaDrawioRequestSource().matchAll(/<mxCell\b([^>]*)/g))
    .map((match) => katanaDrawioXmlAttributes(match[1]))
    .some(
      (attributes) =>
        katanaDrawioCellAttribute(attributes, "id") === id &&
        katanaDrawioCellAttribute(attributes, "edge") === "1",
    );
}

function katanaDrawioGroupHasDirectShape(group) {
  return Array.from(group.children).some((child) =>
    KATANA_DRAWIO_HALF_PIXEL_SHAPE_TAGS.has(child.localName),
  );
}

function katanaApplyDrawioHalfPixelTranslate(group) {
  group.setAttribute("transform", "translate(0.5,0.5)");
}

function katanaDrawioNeedsNearIntegerGeometryRounding() {
  return katanaDrawioRequestSource().includes("mxgraph.infographic");
}

function katanaRoundDrawioNearIntegerPaths(svg) {
  Array.from(svg.querySelectorAll("path"))
    .filter((path) => path.getAttribute("d"))
    .forEach((path) => {
      path.setAttribute("d", katanaDrawioRoundNearIntegerPath(path.getAttribute("d")));
    });
}

function katanaDrawioRoundNearIntegerPath(data) {
  return String(data).replace(/-?\d+\.\d+/g, katanaDrawioRoundNearIntegerNumber);
}

function katanaDrawioRoundNearIntegerNumber(value) {
  const number = Number(value);
  const rounded = Math.round(number);
  return Math.abs(number - rounded) <= 0.051 ? String(rounded) : value;
}

const KATANA_DRAWIO_HALF_PIXEL_SHAPE_TAGS = new Set([
  "circle",
  "ellipse",
  "line",
  "path",
  "polygon",
  "polyline",
  "rect",
]);
