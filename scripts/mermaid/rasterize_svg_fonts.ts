export const SvgSourceFonts = {
  urls(svg: string): string[] {
    return Array.from(String(svg).matchAll(/@import\s+url\("([^"]+)"\)/gi))
      .map((match) => match[1] ?? "")
      .filter((url) => url.startsWith("https://"))
      .filter((url, index, urls) => urls.indexOf(url) === index);
  },
};
