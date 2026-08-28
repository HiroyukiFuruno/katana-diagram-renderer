import { expect, test } from "bun:test";
import { SvgSourceFonts } from "./rasterize_svg_fonts";

test("extracts unique HTTPS font imports", () => {
  const svg = `<svg><style>@import url("https://fonts.example.test/a.css");</style><style>@import url("https://fonts.example.test/a.css");</style><style>@import url("http://fonts.example.test/b.css");</style></svg>`;

  expect(SvgSourceFonts.urls(svg)).toEqual(["https://fonts.example.test/a.css"]);
});
