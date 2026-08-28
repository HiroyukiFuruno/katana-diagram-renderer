function katanaInstallDrawioSourceFonts(svg) {
  const urls = katanaDrawioSourceFontUrls();
  if (urls.length === 0) {
    return;
  }
  const css = katanaDrawioSourceFontCss(urls);
  const defs = svg.querySelector("defs") ?? katanaCreateDrawioSvgDefs(svg);
  defs.appendChild(katanaCreateDrawioSourceFontStyle("http://www.w3.org/2000/svg", css));
  Array.from(svg.querySelectorAll("foreignObject")).forEach((foreignObject) => {
    [...urls].reverse().forEach((url) => {
      foreignObject.insertBefore(katanaCreateDrawioSourceFontLink(url), foreignObject.firstChild);
    });
    foreignObject.insertBefore(
      katanaCreateDrawioSourceFontStyle("http://www.w3.org/1999/xhtml", css),
      foreignObject.firstChild,
    );
  });
}

function katanaCreateDrawioSourceFontLink(url) {
  const namespace = "http://www.w3.org/1999/xhtml";
  const link = document.createElementNS(namespace, "link");
  link.setAttribute("xmlns", namespace);
  link.setAttribute("rel", "stylesheet");
  link.setAttribute("href", url);
  return link;
}

function katanaDrawioSourceFontCss(urls) {
  return urls.map((url) => `@import url(${JSON.stringify(url)});`).join("\n");
}

function katanaCreateDrawioSourceFontStyle(namespace, css) {
  const style = document.createElementNS(namespace, "style");
  if (namespace === "http://www.w3.org/1999/xhtml") {
    style.setAttribute("xmlns", namespace);
  }
  style.setAttribute("type", "text/css");
  style.textContent = css;
  return style;
}

function katanaDrawioSourceFontUrls() {
  return Array.from(katanaDrawioRequestSource().matchAll(/(?:^|[;"])fontSource=([^;"<]+)/g))
    .map((match) => katanaDecodeDrawioFontUrl(match[1]))
    .filter((url) => url.startsWith("https://"))
    .filter((url, index, urls) => urls.indexOf(url) === index);
}

function katanaDecodeDrawioFontUrl(value) {
  try {
    return decodeURIComponent(String(value));
  } catch (_error) {
    return "";
  }
}

function katanaCreateDrawioSvgDefs(svg) {
  const defs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
  svg.insertBefore(defs, svg.firstChild);
  return defs;
}
