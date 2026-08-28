import { expect, test } from "bun:test";
import { brotliDecompressSync } from "node:zlib";
import { compressRuntimePackageAsset } from "./runtime-package-asset-compressor";

test("配布用圧縮資産は決定的かつ元バイト列へ復元できる", () => {
  const source = Buffer.from("const runtimeAsset = '刀'.repeat(1024);\n", "utf8");

  const first = compressRuntimePackageAsset(source);
  const second = compressRuntimePackageAsset(source);

  expect(first.equals(second)).toBe(true);
  expect(brotliDecompressSync(first).equals(source)).toBe(true);
});
