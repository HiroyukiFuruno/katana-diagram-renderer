import fs from "node:fs";
import path from "node:path";
import { brotliCompressSync, constants } from "node:zlib";
import {
  RuntimeAssetCatalog,
  type RuntimeAssetDefinition,
  RuntimeAssetPaths,
} from "./runtime-asset-common";

type CompressionMode = "--check" | "--write";

const COMPRESSED_RUNTIME_ASSET_KINDS = new Set(["mermaid", "drawio"]);
const ZENUML_RUNTIME_ASSET_KINDS = ["zenuml-core", "mermaid-zenuml"] as const;
const DRAWIO_RESOURCE_ROOT = path.join(
  "crates",
  "katana-render-runtime",
  "src",
  "markdown",
  "drawio_renderer",
  "js_runtime",
  "resources",
);
const DRAWIO_RESOURCE_ARCHIVE_EXCLUDED_ROOT_ENTRIES = new Set([
  "drawio-libs",
  "drawio-resource-manifest.json",
  "shapes",
  "stencils",
]);
const DRAWIO_RESOURCE_GENERATED_DIR = path.join(
  "crates",
  "katana-render-runtime",
  "src",
  "markdown",
  "drawio_renderer",
  "generated",
);
const DRAWIO_RESOURCE_ARCHIVE = path.join(DRAWIO_RESOURCE_GENERATED_DIR, "drawio-resources.bin.br");
const DRAWIO_RESOURCE_INDEX = path.join(DRAWIO_RESOURCE_GENERATED_DIR, "drawio-resources-index.rs");
const DRAWIO_RESOURCE_MEDIA_INDEX = path.join(
  DRAWIO_RESOURCE_GENERATED_DIR,
  "drawio-resources-media-index.rs",
);
const DRAWIO_RESOURCE_DATA_INDEX = path.join(
  DRAWIO_RESOURCE_GENERATED_DIR,
  "drawio-resources-data-index.rs",
);
const ZENUML_RUNTIME_ASSET_GENERATED_DIR = path.join(
  "crates",
  "katana-render-runtime",
  "src",
  "markdown",
  "generated",
);
const ZENUML_RUNTIME_ASSET_ARCHIVE = path.join(
  ZENUML_RUNTIME_ASSET_GENERATED_DIR,
  "zenuml-runtime-assets.bin.br",
);
const ZENUML_RUNTIME_ASSET_INDEX = path.join(
  ZENUML_RUNTIME_ASSET_GENERATED_DIR,
  "zenuml-runtime-assets-index.rs",
);

export interface DrawioResourceArchiveFile {
  readonly path: string;
  readonly bytes: Uint8Array;
}

export interface DrawioResourceArchive {
  readonly compressedBytes: Buffer;
  readonly indexSource: string;
  readonly mediaIndexSource: string;
  readonly dataIndexSource: string;
}

export interface ZenumlRuntimeAssetArchiveFile {
  readonly kind: (typeof ZENUML_RUNTIME_ASSET_KINDS)[number];
  readonly bytes: Uint8Array;
}

export interface ZenumlRuntimeAssetArchive {
  readonly compressedBytes: Buffer;
  readonly indexSource: string;
}

export function compressRuntimePackageAsset(bytes: Uint8Array): Buffer {
  return brotliCompressSync(bytes, {
    params: {
      [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
      [constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
    },
  });
}

export function buildDrawioResourceArchive(
  files: readonly DrawioResourceArchiveFile[],
): DrawioResourceArchive {
  const sorted = [...files].sort((left, right) => left.path.localeCompare(right.path));
  const contents: Buffer[] = [];
  const mediaEntries: string[] = [];
  const dataEntries: string[] = [];
  let offset = 0;
  for (const file of sorted) {
    const bytes = Buffer.from(file.bytes);
    contents.push(bytes);
    const entry = `    (${JSON.stringify(file.path)}, ${offset}, ${bytes.length}),`;
    (file.path.startsWith("data/") ? dataEntries : mediaEntries).push(entry);
    offset += bytes.length;
  }
  const rawBytes = Buffer.concat(contents);
  const indexSource = [
    `pub(super) const DRAWIO_RESOURCE_ARCHIVE_UNCOMPRESSED_LENGTH: usize = ${rawBytes.length};`,
    'include!("drawio-resources-media-index.rs");',
    'include!("drawio-resources-data-index.rs");',
    "pub(super) const DRAWIO_RESOURCE_ARCHIVE_INDEXES: &[DrawioResourceArchiveIndex] = &[",
    "    DRAWIO_RESOURCE_ARCHIVE_MEDIA_INDEX,",
    "    DRAWIO_RESOURCE_ARCHIVE_DATA_INDEX,",
    "];",
    "",
  ].join("\n");
  return {
    compressedBytes: compressRuntimePackageAsset(rawBytes),
    indexSource,
    mediaIndexSource: indexArraySource("DRAWIO_RESOURCE_ARCHIVE_MEDIA_INDEX", mediaEntries),
    dataIndexSource: indexArraySource("DRAWIO_RESOURCE_ARCHIVE_DATA_INDEX", dataEntries),
  };
}

function indexArraySource(name: string, entries: readonly string[]): string {
  return [
    `pub(super) const ${name}: &[DrawioResourceArchiveEntry] = &[`,
    ...entries,
    "];",
    "",
  ].join("\n");
}

export function buildZenumlRuntimeAssetArchive(
  files: readonly ZenumlRuntimeAssetArchiveFile[],
): ZenumlRuntimeAssetArchive {
  const contents: Buffer[] = [];
  const constants: string[] = [];
  let offset = 0;
  for (const kind of ZENUML_RUNTIME_ASSET_KINDS) {
    const file = files.find((it) => it.kind === kind);
    if (file === undefined) {
      throw new Error(`ZenUML runtime asset is missing: ${kind}`);
    }
    const bytes = Buffer.from(file.bytes);
    const prefix = kind.toUpperCase().replaceAll("-", "_");
    contents.push(bytes);
    if (kind === "zenuml-core") {
      constants.push("#[cfg(test)]");
    }
    constants.push(`pub(super) const ${prefix}_ASSET_OFFSET: usize = ${offset};`);
    if (kind === "zenuml-core") {
      constants.push("#[cfg(test)]");
    }
    constants.push(`pub(super) const ${prefix}_ASSET_LENGTH: usize = ${bytes.length};`);
    offset += bytes.length;
  }
  const rawBytes = Buffer.concat(contents);
  return {
    compressedBytes: compressRuntimePackageAsset(rawBytes),
    indexSource: [
      ...constants,
      `pub(super) const ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH: usize = ${rawBytes.length};`,
      "",
    ].join("\n"),
  };
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
    this.syncDrawioResources(mode);
    this.syncZenumlRuntimeAssets(mode);
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

  private syncDrawioResources(mode: CompressionMode): void {
    const archive = buildDrawioResourceArchive(this.drawioResourceFiles(DRAWIO_RESOURCE_ROOT));
    this.syncFile(mode, DRAWIO_RESOURCE_ARCHIVE, archive.compressedBytes);
    this.syncFile(mode, DRAWIO_RESOURCE_INDEX, Buffer.from(archive.indexSource, "utf8"));
    this.syncFile(mode, DRAWIO_RESOURCE_MEDIA_INDEX, Buffer.from(archive.mediaIndexSource, "utf8"));
    this.syncFile(mode, DRAWIO_RESOURCE_DATA_INDEX, Buffer.from(archive.dataIndexSource, "utf8"));
  }

  private syncZenumlRuntimeAssets(mode: CompressionMode): void {
    const archive = buildZenumlRuntimeAssetArchive(
      ZENUML_RUNTIME_ASSET_KINDS.map((kind) => {
        const definition = RuntimeAssetCatalog.byKind(kind);
        return {
          kind,
          bytes: fs.readFileSync(RuntimeAssetPaths.assetFile(definition)),
        };
      }),
    );
    this.syncFile(mode, ZENUML_RUNTIME_ASSET_ARCHIVE, archive.compressedBytes);
    this.syncFile(mode, ZENUML_RUNTIME_ASSET_INDEX, Buffer.from(archive.indexSource, "utf8"));
  }

  private drawioResourceFiles(root: string): DrawioResourceArchiveFile[] {
    return this.walkFiles(root)
      .filter((filePath) => {
        const rootEntry = path.relative(root, filePath).split(path.sep).at(0);
        return (
          rootEntry !== undefined && !DRAWIO_RESOURCE_ARCHIVE_EXCLUDED_ROOT_ENTRIES.has(rootEntry)
        );
      })
      .map((filePath) => ({
        path: path.relative(root, filePath).split(path.sep).join("/"),
        bytes: fs.readFileSync(filePath),
      }));
  }

  private walkFiles(directory: string): string[] {
    return fs
      .readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name))
      .flatMap((entry) => {
        const entryPath = path.join(directory, entry.name);
        return entry.isDirectory() ? this.walkFiles(entryPath) : [entryPath];
      });
  }

  private syncFile(mode: CompressionMode, target: string, expected: Buffer): void {
    if (mode === "--write") {
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, expected);
      return;
    }
    const actual = fs.existsSync(target) ? fs.readFileSync(target) : undefined;
    if (actual === undefined || !actual.equals(expected)) {
      throw new Error(`Compressed runtime package asset is stale: ${target}`);
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
