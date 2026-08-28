import { LatestVersionClient } from "./latest-check";
import { RuntimeAssetCatalog, type RuntimeAssetDefinition } from "./runtime-asset-common";

export type DependencyCommandRunner = (command: string, args: readonly string[]) => Promise<void>;

interface RuntimeAssetLatestClient {
  latest(definition: RuntimeAssetDefinition): Promise<string>;
}

class BunCommandRunner {
  async run(command: string, args: readonly string[]): Promise<void> {
    const process = Bun.spawn([command, ...args], {
      stdout: "inherit",
      stderr: "inherit",
    });
    if ((await process.exited) !== 0) {
      throw new Error(`Command failed: ${command} ${args.join(" ")}`);
    }
  }
}

export class DependencyUpdateAllCommand {
  constructor(
    private readonly definitions: readonly RuntimeAssetDefinition[] = RuntimeAssetCatalog.all(),
    private readonly client: RuntimeAssetLatestClient = new LatestVersionClient(),
    private readonly runner: DependencyCommandRunner = (command, args) =>
      new BunCommandRunner().run(command, args),
    private readonly report: (message: string) => void = console.log,
  ) {}

  async run(): Promise<void> {
    for (const definition of this.definitions) {
      await this.updateRuntimeAsset(definition);
    }
    await this.runner("bun", [
      "run",
      "scripts/runtime-assets/runtime-package-asset-compressor.ts",
      "--write",
    ]);
  }

  private async updateRuntimeAsset(definition: RuntimeAssetDefinition): Promise<void> {
    const latest = await this.client.latest(definition);
    if (latest === definition.version) {
      this.report(`${definition.displayName}: already latest (${latest})`);
      return;
    }
    this.report(`${definition.displayName}: ${definition.version} -> ${latest}`);
    await this.runner("bun", ["run", "scripts/runtime-assets/update.ts", definition.kind, latest]);
  }
}

if (import.meta.main) {
  await new DependencyUpdateAllCommand().run();
}
