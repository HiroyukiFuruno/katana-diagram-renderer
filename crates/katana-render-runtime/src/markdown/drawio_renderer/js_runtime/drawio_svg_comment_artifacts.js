function katanaRemoveDrawioCommentArtifacts(svg) {
  Array.from(svg.querySelectorAll("switch"))
    .filter(katanaIsDrawioCommentArtifact)
    .forEach(katanaRemoveDrawioNode);
}

function katanaIsDrawioCommentArtifact(node) {
  const text = String(node.textContent).replace(/\s+/g, "");
  return text.length > 0 && text.replace(/!-->/g, "").length === 0;
}
