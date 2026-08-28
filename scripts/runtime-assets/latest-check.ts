import type { RuntimeAssetDefinition } from "./runtime-asset-common";

interface NpmLatestResponse {
  readonly version: string;
}

interface GitHubLatestResponse {
  readonly tag_name: string;
}

export type RuntimeAssetFetch = (url: string, init: RequestInit) => Promise<Response>;

export class LatestVersionClient {
  constructor(private readonly fetcher: RuntimeAssetFetch = (url, init) => fetch(url, init)) {}

  async latest(definition: RuntimeAssetDefinition): Promise<string> {
    if (definition.kind === "drawio") {
      return this.drawio(definition.latestUrl);
    }
    if (definition.kind === "plantuml") {
      return this.plantuml(definition.latestUrl);
    }
    return this.npm(definition.latestUrl);
  }

  private async npm(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as NpmLatestResponse;
    return body.version;
  }

  private async drawio(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as GitHubLatestResponse;
    return body.tag_name.replace(/^v/, "");
  }

  private async plantuml(url: string): Promise<string> {
    const response = await this.get(url);
    const body = await response.text();
    const versions = [...body.matchAll(/<version>([^<]+)<\/version>/g)].map((it) => it[1]);
    const latest = versions.at(-1);
    if (latest === undefined) {
      throw new Error(`PlantUML metadata did not include versions: ${url}`);
    }
    return latest;
  }

  private async get(url: string): Promise<Response> {
    const response = await this.fetcher(url, {
      headers: {
        accept: "application/json",
        "user-agent": "katana-render-runtime-release-tool",
      },
    });
    if (!response.ok) {
      throw new Error(`Failed to fetch ${url}: ${response.status}`);
    }
    return response;
  }
}
