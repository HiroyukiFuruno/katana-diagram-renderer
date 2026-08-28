import fs from "node:fs";
import { brotliCompressSync, constants } from "node:zlib";
import {
  RuntimeAssetCatalog,
  type RuntimeAssetDefinition,
  RuntimeAssetPaths,
} from "./runtime-asset-common";

type CompressionMode = "--check" | "--write";

const COMPRESSED_RUNTIME_ASSET_KINDS = new Set(["mermaid", "drawio"]);

export function compressRuntimePackageAsset(bytes: Uint8Array): Buffer {
  return brotliCompressSync(bytes, {
    params: {
      [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
      [constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
    },
  });
}

export class RuntimePackageAssetCompressor {
  static supports(definition: RuntimeAssetDefinition): boolean {
    return COMPRESSED_RUNTIME_ASSET_KINDS.has(definition.kind);
  }

  write(definition: RuntimeAssetDefinition, version = definition.version): void {
    this.assertSupported(definition);
    const compressed = this.compressedBytes(definition, version);
    fs.writeFileSync(RuntimeAssetPaths.compressedAssetFile(definition, version), compressed);
  }

  check(definition: RuntimeAssetDefinition, version = definition.version): void {
    this.assertSupported(definition);
    const target = RuntimeAssetPaths.compressedAssetFile(definition, version);
    const expected = this.compressedBytes(definition, version);
    const actual = fs.existsSync(target) ? fs.readFileSync(target) : undefined;
    if (actual === undefined || !actual.equals(expected)) {
      throw new Error(`Compressed runtime package asset is stale: ${target}`);
    }
  }

  run(mode: CompressionMode): void {
    for (const definition of RuntimeAssetCatalog.all().filter((it) =>
      RuntimePackageAssetCompressor.supports(it),
    )) {
      if (mode === "--write") {
        this.write(definition);
      } else {
        this.check(definition);
      }
    }
  }

  private compressedBytes(definition: RuntimeAssetDefinition, version: string): Buffer {
    return compressRuntimePackageAsset(
      fs.readFileSync(RuntimeAssetPaths.assetFile(definition, version)),
    );
  }

  private assertSupported(definition: RuntimeAssetDefinition): void {
    if (!RuntimePackageAssetCompressor.supports(definition)) {
      throw new Error(`Runtime asset is not package-compressed: ${definition.kind}`);
    }
  }
}

function compressionMode(argv: readonly string[]): CompressionMode {
  const mode = argv.at(0);
  if (argv.length !== 1 || (mode !== "--check" && mode !== "--write")) {
    throw new Error("Usage: runtime-package-asset-compressor.ts <--check|--write>");
  }
  return mode;
}

if (import.meta.main) {
  new RuntimePackageAssetCompressor().run(compressionMode(process.argv.slice(2)));
}
