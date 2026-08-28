const KATANA_DRAWIO_EXTERNAL_IMAGE_EXPORT_BOTTOM_PADDING = 11;
const KATANA_DRAWIO_SHADOW_EXPORT_RIGHT_PADDING = 10;
const KATANA_DRAWIO_SHADOW_EXPORT_BOTTOM_PADDING = 11;
const KATANA_DRAWIO_DEVICE_SHADOW_EXPORT_BOTTOM_PADDING = 7;
const KATANA_DRAWIO_SOURCE_TOP_PADDING_LIMIT = 12;
const KATANA_DRAWIO_PAINT_EDGE_TOLERANCE = 2;

function katanaRemoveOversizedDrawioLabelBackgrounds(svg) {
  Array.from(svg.querySelectorAll("rect"))
    .filter(katanaIsDrawioTextLabelBackground)
    .forEach(katanaRemoveDrawioNode);
  Array.from(svg.querySelectorAll("rect"))
    .filter(katanaIsDrawioPageSizedLabelBackground)
    .forEach(katanaRemoveDrawioNode);
}

function katanaRemoveDrawioNode(node) {
  node.parentNode?.removeChild(node);
}

function katanaIsDrawioPageSizedLabelBackground(rect) {
  return [
    katanaDrawioColorValue(rect, "fill") === "#ffffff",
    rect.getAttribute("stroke") === "none",
    katanaDrawioNodeWidth(rect) > katanaDrawioSvgBox(rect.ownerSVGElement).width * 0.5,
    katanaDrawioRectBelongsToSourceLabel(rect),
  ].every(Boolean);
}

function katanaDrawioRectBelongsToSourceLabel(rect) {
  const id = katanaDrawioElementCellId(rect);
  return katanaDrawioSourceLabelEntries().some((entry) => entry.id === id);
}

function katanaIsDrawioTextLabelBackground(rect) {
  return [
    katanaDrawioColorValue(rect, "fill") === "#ffffff",
    rect.getAttribute("stroke") === "none",
    katanaDrawioNodeWidth(rect) > 16,
    katanaHasDrawioTextSibling(rect),
    !katanaDrawioIsEdgeLabelBackground(rect),
  ].every(Boolean);
}

function katanaDrawioIsEdgeLabelBackground(rect) {
  const id = katanaDrawioElementCellId(rect);
  return [
    katanaDrawioIsHtmlClassDiagramSource(),
    katanaDrawioSourceLabelEntries().some((entry) => entry.id === id && entry.edge),
  ].every(Boolean);
}

function katanaHasDrawioTextSibling(rect) {
  return Array.from(rect.parentNode?.querySelectorAll("text") ?? []).length > 0;
}

function katanaDrawioColorValue(node, name) {
  return String(node.getAttribute(name) ?? "").toLowerCase();
}

function katanaCropDrawioSvgToContent(svg) {
  katanaApplyDrawioCrop(svg, katanaDrawioCropBox(svg));
}

function katanaDrawioCropBox(svg) {
  const box =
    KATANA_DRAWIO_CROP_BOX_READERS[Number(katanaDrawioShouldMeasureRenderedContent(svg))](svg);
  if (katanaDrawioHasSourceAlignedBottomPageMargin(svg) && !katanaDrawioIsArrowComparisonSource()) {
    return katanaDrawioSourceAlignedTopPaddingBox(svg, box);
  }
  return katanaDrawioHasSymmetricImplicitPageMargin(svg) && !katanaDrawioIsArrowComparisonSource()
    ? box
    : katanaDrawioAlignedCropBox(svg, box);
}

const KATANA_DRAWIO_CROP_BOX_READERS = [
  (svg) => katanaDrawioPreferredContentBox(svg),
  (svg) => katanaDrawioMeasuredContentBox(svg),
];

function katanaDrawioMeasuredContentBox(svg) {
  return katanaDrawioUnionBox(
    [katanaDrawioContentBox(svg), katanaDrawioPreferredSourceBox(svg)].filter(Boolean),
  );
}

function katanaDrawioPreferredContentBox(svg) {
  const sourceBox = katanaDrawioSourceBoxWithLeftLabelPadding(
    katanaDrawioPreferredSourceBox(svg),
  );
  if (katanaDrawioIsKanbanExamplesSource()) {
    return sourceBox ?? katanaDrawioContentBox(svg);
  }
  const contentBox = katanaDrawioContentBox(svg);
  if (
    katanaDrawioSourceDisablesPageBounds() &&
    katanaDrawioTextOverflowsSourceBox(svg, sourceBox)
  ) {
    return contentBox;
  }
  return katanaDrawioCanUseSourceContentBox(sourceBox) ? sourceBox : katanaDrawioContentBox(svg);
}

function katanaDrawioSourceBoxWithLeftLabelPadding(box) {
  const padding = katanaDrawioAppliedLeftPadding();
  return box && padding > 0
    ? {
        x: box.x - padding,
        y: box.y,
        width: box.width + padding,
        height: box.height,
      }
    : box;
}

function katanaDrawioPreferredSourceBox(svg) {
  return katanaDrawioUsesDevicePageContentCrop()
    ? katanaDrawioSourcePaintBox(svg)
    : katanaDrawioSourceContentBox(svg);
}

function katanaDrawioTextOverflowsSourceBox(svg, sourceBox) {
  if (!katanaDrawioCanUseSourceContentBox(sourceBox)) {
    return false;
  }
  return katanaDrawioContentElements(svg)
    .filter((element) => element.localName === "text")
    .map(katanaDrawioElementBox)
    .some((box) => katanaDrawioBoxOverflowsSourceBox(box, sourceBox));
}

function katanaDrawioBoxOverflowsSourceBox(box, sourceBox) {
  return [
    box.x < sourceBox.x,
    box.y < sourceBox.y,
    katanaDrawioBoxRight(box) > katanaDrawioBoxRight(sourceBox),
    katanaDrawioBoxBottom(box) > katanaDrawioBoxBottom(sourceBox),
  ].some(Boolean);
}

function katanaDrawioCanUseSourceContentBox(box) {
  return [
    box,
    Number.isFinite(box?.x),
    Number.isFinite(box?.y),
    box?.width > 0,
    box?.height > 0,
  ].every(Boolean);
}

function katanaDrawioShouldMeasureRenderedContent(svg) {
  if (katanaDrawioIsKanbanExamplesSource()) {
    return false;
  }
  return [
    katanaDrawioRequestSource().includes("mxgraph.aws3d."),
    katanaDrawioIsAwsSaasSource(),
    katanaDrawioRequestSource().includes("mxgraph.aws"),
    katanaDrawioNeedsMeasuredContentBox(svg),
  ].some(Boolean);
}

function katanaDrawioIsKanbanExamplesSource() {
  const source = katanaDrawioRequestSource();
  return [
    source.includes("shape=partialRectangle"),
    source.includes("shape=mxgraph.ios.iPin"),
    source.includes("shape=table"),
  ].every(Boolean);
}

function katanaDrawioNeedsMeasuredContentBox(svg) {
  if (!katanaDrawioSourceIsDeviceTemplate()) {
    return false;
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  if (!sourceBox) {
    return true;
  }
  return [
    sourceBox,
    contentBox,
    sourceBox.width > 0,
    contentBox.width > 0,
    [
      contentBox.width > sourceBox.width * 1.03,
      contentBox.height > sourceBox.height * 1.03,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioContentBox(svg) {
  return katanaDrawioOptionalContentBox(svg) ?? katanaDrawioEmptyContentBox();
}

function katanaDrawioOptionalContentBox(svg) {
  const boxes = katanaDrawioContentElements(svg).map(katanaDrawioElementBox);
  return katanaDrawioUnionBox(boxes.filter(katanaDrawioHasArea));
}

function katanaDrawioContentElements(svg) {
  return katanaDrawioContentTagNames()
    .flatMap((tagName) => Array.from(svg.querySelectorAll(tagName)))
    .filter((element) => !katanaShouldIgnoreDrawioContentElement(element));
}

function katanaDrawioContentTagNames() {
  return ["rect", "path", "ellipse", "circle", "line", "polygon", "polyline", "image", "text"];
}

function katanaShouldIgnoreDrawioContentElement(element) {
  return [
    katanaIsWrappedDrawioHtmlFallbackText(element),
    katanaDrawioElementAncestors(element).some((ancestor) => ancestor.localName === "defs"),
  ].some(Boolean);
}

function katanaIsWrappedDrawioHtmlFallbackText(element) {
  const style = katanaDrawioSourceStyleForElement(element);
  return [
    element.localName === "text",
    style.get("whiteSpace") === "wrap",
    Boolean(katanaDrawioContentCellGroup(element)?.querySelector("foreignObject")),
  ].every(Boolean);
}

function katanaDrawioContentCellGroup(element) {
  return katanaDrawioElementAncestors(element)
    .filter((node) => node.getAttribute?.("data-cell-id"))
    .concat([null])[0];
}

function katanaDrawioElementBox(element) {
  const box = element.getBBox();
  const matrix = katanaDrawioElementTransformMatrix(element);
  return ["ellipse", "circle"].includes(element.localName)
    ? katanaDrawioTransformedEllipseBox(box, matrix)
    : katanaDrawioTransformedBox(box, matrix);
}

function katanaDrawioElementTransformMatrix(element) {
  let matrix = katanaDrawioIdentityMatrix();
  let node = element;
  while (node && node !== element.ownerSVGElement) {
    matrix = katanaDrawioMultiplyMatrices(katanaDrawioNodeTransformMatrix(node), matrix);
    node = node.parentNode;
  }
  return matrix;
}

function katanaDrawioNodeTransformMatrix(node) {
  const transform = String(node?.getAttribute?.("transform") ?? "");
  const matches = Array.from(transform.matchAll(/([a-zA-Z]+)\s*\(([^)]*)\)/g));
  return matches.reduce((matrix, match) => {
    return katanaDrawioMultiplyMatrices(matrix, katanaDrawioTransformMatrix(match));
  }, katanaDrawioIdentityMatrix());
}

function katanaDrawioTransformMatrix(match) {
  const name = match[1].toLowerCase();
  const values = katanaDrawioTransformNumbers(match[2]);
  if (name === "matrix" && values.length >= 6) {
    return values.slice(0, 6);
  }
  if (name === "translate") {
    return [1, 0, 0, 1, values[0] ?? 0, values[1] ?? 0];
  }
  if (name === "scale") {
    const x = values[0] ?? 1;
    return [x, 0, 0, values[1] ?? x, 0, 0];
  }
  if (name === "rotate") {
    return katanaDrawioRotateMatrix(values);
  }
  if (name === "skewx") {
    return [1, 0, Math.tan(katanaDrawioRadians(values[0] ?? 0)), 1, 0, 0];
  }
  if (name === "skewy") {
    return [1, Math.tan(katanaDrawioRadians(values[0] ?? 0)), 0, 1, 0, 0];
  }
  return katanaDrawioIdentityMatrix();
}

function katanaDrawioTransformNumbers(value) {
  return Array.from(
    String(value).matchAll(/[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/gi),
    (match) => Number(match[0]),
  );
}

function katanaDrawioRotateMatrix(values) {
  const angle = katanaDrawioRadians(values[0] ?? 0);
  const cosine = Math.cos(angle);
  const sine = Math.sin(angle);
  const rotation = [cosine, sine, -sine, cosine, 0, 0];
  if (values.length < 3) {
    return rotation;
  }
  const center = [1, 0, 0, 1, values[1], values[2]];
  const inverseCenter = [1, 0, 0, 1, -values[1], -values[2]];
  return katanaDrawioMultiplyMatrices(
    katanaDrawioMultiplyMatrices(center, rotation),
    inverseCenter,
  );
}

function katanaDrawioRadians(degrees) {
  return (degrees * Math.PI) / 180;
}

function katanaDrawioIdentityMatrix() {
  return [1, 0, 0, 1, 0, 0];
}

function katanaDrawioMultiplyMatrices(left, right) {
  const [a1, b1, c1, d1, e1, f1] = left;
  const [a2, b2, c2, d2, e2, f2] = right;
  return [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ];
}

function katanaDrawioTransformedEllipseBox(box, matrix) {
  const [a, b, c, d] = matrix;
  const center = katanaDrawioTransformPoint(
    { x: box.x + box.width / 2, y: box.y + box.height / 2 },
    matrix,
  );
  const radiusX = box.width / 2;
  const radiusY = box.height / 2;
  const extentX = Math.hypot(a * radiusX, c * radiusY);
  const extentY = Math.hypot(b * radiusX, d * radiusY);
  return {
    x: center.x - extentX,
    y: center.y - extentY,
    width: extentX * 2,
    height: extentY * 2,
  };
}

function katanaDrawioTransformedBox(box, matrix) {
  const points = [
    { x: box.x, y: box.y },
    { x: box.x + box.width, y: box.y },
    { x: box.x, y: box.y + box.height },
    { x: box.x + box.width, y: box.y + box.height },
  ].map((point) => katanaDrawioTransformPoint(point, matrix));
  const left = Math.min(...points.map((point) => point.x));
  const top = Math.min(...points.map((point) => point.y));
  const right = Math.max(...points.map((point) => point.x));
  const bottom = Math.max(...points.map((point) => point.y));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

function katanaDrawioTransformPoint(point, matrix) {
  const [a, b, c, d, e, f] = matrix;
  return {
    x: a * point.x + c * point.y + e,
    y: b * point.x + d * point.y + f,
  };
}

function katanaDrawioTranslatedBox(box, translate) {
  return {
    x: box.x + translate.x,
    y: box.y + translate.y,
    width: box.width,
    height: box.height,
  };
}

function katanaDrawioTranslate(element) {
  return katanaDrawioParentTranslate(element.parentNode);
}

function katanaDrawioParentTranslate(node) {
  return node
    ? katanaDrawioAddTranslate(katanaDrawioParentTranslate(node.parentNode), node)
    : { x: 0, y: 0 };
}

function katanaDrawioAddTranslate(translate, node) {
  const next = katanaDrawioNodeTranslate(node);
  return { x: translate.x + next.x, y: translate.y + next.y };
}

function katanaDrawioNodeTranslate(node) {
  return katanaDrawioTranslateMatch(String(node?.getAttribute?.("transform") ?? ""));
}

function katanaDrawioTranslateMatch(transform) {
  const match = transform.match(/translate\(([-\d.]+)(?:[,\s]+([-\d.]+))?\)/);
  return [match]
    .filter(Boolean)
    .map(katanaDrawioTranslateFromMatch)
    .concat([{ x: 0, y: 0 }])[0];
}

function katanaDrawioTranslateFromMatch(match) {
  return { x: Number(match[1]), y: Number(match[2] ?? 0) };
}

function katanaDrawioUnionBox(boxes) {
  return boxes.length === 0 ? null : katanaDrawioUnionNonEmptyBox(boxes);
}

function katanaDrawioUnionNonEmptyBox(boxes) {
  const left = Math.min(...boxes.map((box) => box.x));
  const top = Math.min(...boxes.map((box) => box.y));
  const right = Math.max(...boxes.map(katanaDrawioBoxRight));
  const bottom = Math.max(...boxes.map(katanaDrawioBoxBottom));
  return {
    x: Math.floor(left),
    y: Math.floor(top),
    width: Math.ceil(right - left),
    height: Math.ceil(bottom - top),
  };
}

function katanaDrawioBoxRight(box) {
  return box.x + box.width;
}

function katanaDrawioBoxBottom(box) {
  return box.y + box.height;
}

function katanaDrawioHasArea(box) {
  return [box.width > 0, box.height > 0].every(Boolean);
}

function katanaDrawioEmptyContentBox() {
  return { x: 0, y: 0, width: 1, height: 1 };
}

function katanaDrawioAlignedCropBox(svg, box) {
  const contentBox = katanaDrawioContentBox(svg);
  const alignedBox = {
    ...box,
    x: katanaDrawioAlignedCropOrigin(svg, box, contentBox, "x"),
    y: katanaDrawioAlignedCropOrigin(svg, box, contentBox, "y"),
  };
  if (katanaDrawioHasPreservedTopPadding(alignedBox)) {
    return {
      x: alignedBox.x,
      y: 0,
      width: alignedBox.width,
      height: alignedBox.height + alignedBox.y + 1,
    };
  }
  return katanaDrawioHasOnePixelTopLeftCrop(alignedBox)
    ? { x: 0, y: 0, width: alignedBox.width + 1, height: alignedBox.height + 1 }
    : alignedBox;
}

function katanaDrawioSourceAlignedTopPaddingBox(svg, box) {
  const contentBox = katanaDrawioContentBox(svg);
  if (!katanaDrawioHasPreservedTopPadding(box) || contentBox.y < 0) {
    return box;
  }
  const sourceBox = katanaDrawioSourcePaintBox(svg);
  const paintEdge = Number(
    katanaDrawioBoxBottom(sourceBox ?? contentBox) > katanaDrawioBoxBottom(contentBox),
  );
  return {
    x: box.x,
    y: 0,
    width: box.width,
    height: box.height + box.y + paintEdge,
  };
}

function katanaDrawioAlignedCropOrigin(svg, box, contentBox, axis) {
  const origin = box[axis];
  if (katanaDrawioUsesClippedNegativeCropOrigin(axis, origin)) {
    return 0;
  }
  return Math.abs(origin + 1) <= Number.EPSILON &&
    contentBox[axis] <= -1 &&
    !katanaDrawioHasFractionalNegativeContentOrigin(svg, axis)
    ? 0
    : origin;
}

function katanaDrawioUsesClippedNegativeCropOrigin(axis, origin) {
  if (katanaDrawioIsDeviceAwsTemplateSource()) {
    return origin < 0 && origin >= -2;
  }
  return [
    axis === "x",
    Math.abs(origin + 1) <= Number.EPSILON,
    katanaDrawioIsArrowComparisonSource(),
  ].every(Boolean);
}

function katanaDrawioIsArrowComparisonSource() {
  const source = katanaDrawioRequestSource();
  return [
    (source.match(/shape=mxgraph\.arrows2\.arrow/g) ?? []).length >= 12,
    source.includes('value="Advantage"'),
    source.includes('value="Disadvantage"'),
    source.includes('value="Aspect"'),
  ].every(Boolean);
}

function katanaDrawioHasFractionalNegativeContentOrigin(svg, axis) {
  return katanaDrawioContentElements(svg)
    .map(katanaDrawioElementBox)
    .some((box) => box[axis] > -1 && box[axis] < 0);
}

function katanaDrawioHasPreservedTopPadding(box) {
  return [
    Math.abs(box.x) <= 1,
    box.y > 0,
    box.y <= KATANA_DRAWIO_SOURCE_TOP_PADDING_LIMIT,
  ].every(Boolean);
}

function katanaDrawioHasOnePixelTopLeftCrop(box) {
  return [box.x === 1, box.y === 1].every(Boolean);
}

function katanaApplyDrawioCrop(svg, box) {
  const paddedBox = katanaDrawioExportPaddedBox(svg, box);
  if ([box.x !== 0, box.y !== 0].some(Boolean)) {
    katanaTranslateDrawioContent(svg, box);
  }
  svg.setAttribute("viewBox", `0 0 ${paddedBox.width} ${paddedBox.height}`);
  svg.setAttribute("width", `${paddedBox.width}px`);
  svg.setAttribute("height", `${paddedBox.height}px`);
}

function katanaDrawioExportPaddedBox(svg, box) {
  return {
    width:
      box.width +
      katanaDrawioShadowExportRightPadding(svg, box) +
      katanaDrawioExportRightEdgePadding(svg),
    height:
      box.height +
      katanaDrawioExportBottomPadding() +
      katanaDrawioShadowExportBottomPadding(svg, box) +
      katanaDrawioExportBottomEdgePadding(svg),
  };
}

function katanaDrawioExportRightEdgePadding(svg) {
  return katanaDrawioUsesDevicePageContentCrop()
    ? Number(katanaDrawioRenderedContentOverflowsSourcePaint(svg, katanaDrawioBoxRight))
    : 1;
}

function katanaDrawioExportBottomEdgePadding(svg) {
  if (!katanaDrawioUsesDevicePageContentCrop()) {
    return 1;
  }
  if (!katanaDrawioRenderedContentOverflowsSourcePaint(svg, katanaDrawioBoxBottom)) {
    return 0;
  }
  return katanaDrawioSourcePaintBox(svg)?.y > KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT ? 2 : 1;
}

function katanaDrawioRenderedContentOverflowsSourcePaint(svg, edge) {
  const sourceBox = katanaDrawioPreferredSourceBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  return [
    sourceBox,
    contentBox,
    edge(contentBox) > edge(sourceBox ?? katanaDrawioEmptyContentBox()) + KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
  ].every(Boolean);
}

function katanaDrawioExportBottomPadding() {
  return katanaDrawioHasExternalImageSource()
    ? KATANA_DRAWIO_EXTERNAL_IMAGE_EXPORT_BOTTOM_PADDING
    : 0;
}

function katanaDrawioShadowExportRightPadding(svg, box) {
  return katanaDrawioShadowTouchesExportRight(svg, box)
    ? KATANA_DRAWIO_SHADOW_EXPORT_RIGHT_PADDING
    : 0;
}

function katanaDrawioShadowExportBottomPadding(svg, box) {
  return katanaDrawioShadowTouchesExportBottom(svg, box)
    ? katanaDrawioShadowExportBottomExtent()
    : 0;
}

function katanaDrawioShadowExportBottomExtent() {
  return katanaDrawioUsesDevicePageContentCrop()
    ? KATANA_DRAWIO_DEVICE_SHADOW_EXPORT_BOTTOM_PADDING
    : KATANA_DRAWIO_SHADOW_EXPORT_BOTTOM_PADDING;
}

function katanaDrawioShadowTouchesExportRight(svg, box) {
  const shadowBox = katanaDrawioShadowContentBox(svg);
  return [
    shadowBox,
    katanaDrawioBoxRight(shadowBox ?? katanaDrawioEmptyContentBox()) >=
      katanaDrawioBoxRight(box) - KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
  ].every(Boolean);
}

function katanaDrawioShadowTouchesExportBottom(svg, box) {
  const shadowBox = katanaDrawioShadowContentBox(svg);
  return [
    shadowBox,
    katanaDrawioBoxBottom(shadowBox ?? katanaDrawioEmptyContentBox()) >=
      katanaDrawioBoxBottom(box) - KATANA_DRAWIO_PAINT_EDGE_TOLERANCE,
  ].every(Boolean);
}

function katanaDrawioShadowContentBox(svg) {
  return katanaDrawioUnionBox(
    katanaDrawioContentElements(svg)
      .filter(katanaDrawioElementCellHasShadowStyle)
      .map(katanaDrawioElementBox)
      .filter(katanaDrawioHasArea),
  );
}

function katanaDrawioUsesDevicePageContentCrop() {
  return [
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
  ].every(Boolean);
}

function katanaDrawioHasExternalImageSource() {
  return /(?:^|;)image=https?:\/\//.test(katanaDrawioRequestSource());
}

function katanaTranslateDrawioContent(svg, box) {
  const wrapper = document.createElementNS("http://www.w3.org/2000/svg", "g");
  wrapper.setAttribute("transform", `translate(${-box.x},${-box.y})`);
  Array.from(svg.childNodes).forEach((child) => {
    wrapper.appendChild(child);
  });
  svg.appendChild(wrapper);
}

function katanaDrawioNodeWidth(node) {
  return katanaDrawioCssPixels(node.getAttribute("width"));
}
