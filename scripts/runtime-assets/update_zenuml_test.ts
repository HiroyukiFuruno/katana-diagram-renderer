import { expect, test } from "bun:test";
import { RuntimeAssetCatalog } from "./runtime-asset-common";
import { RuntimeSourceUpdater } from "./update";

test("mermaid-zenuml の圧縮 vendor include path を更新できる", () => {
  const source = [
    "bytes: include_bytes!(",
    '    "../../vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js.br"',
    "),",
  ].join("\n");

  const updated = new RuntimeSourceUpdater().replaceVendorAssetVersion(
    source,
    RuntimeAssetCatalog.byKind("mermaid-zenuml"),
    "0.2.3",
  );

  expect(updated).toContain("../../vendor/mermaid-zenuml/0.2.3/mermaid-zenuml.min.js.br");
});

test("mermaid-zenuml の Cargo package include path を更新できる", () => {
  const source = [
    "include = [",
    '    "vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js.sha256",',
    "]",
  ].join("\n");

  const updated = new RuntimeSourceUpdater().replacePackageIncludeVersion(
    source,
    RuntimeAssetCatalog.byKind("mermaid-zenuml"),
    "0.2.3",
  );

  expect(updated).toContain('"vendor/mermaid-zenuml/0.2.3/mermaid-zenuml.min.js.sha256",');
});

test("zenuml-core の圧縮 asset include path を更新できる", () => {
  const source = ["include = [", '    "vendor/zenuml-core/3.47.8/zenuml.js.sha256",', "]"].join(
    "\n",
  );

  const updated = new RuntimeSourceUpdater().replacePackageIncludeVersion(
    source,
    RuntimeAssetCatalog.byKind("zenuml-core"),
    "3.47.9",
  );

  expect(updated).toContain('"vendor/zenuml-core/3.47.9/zenuml.js.sha256",');
});
