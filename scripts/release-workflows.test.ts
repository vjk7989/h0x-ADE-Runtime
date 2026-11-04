import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const workflow = (name: string) => new URL(`../.github/workflows/${name}.yml`, import.meta.url);
const load = (name: string): any => Bun.YAML.parse(readFileSync(workflow(name), "utf8"));
const release = load("release");
const ci = load("ci");

describe("official publishing workflow boundaries", () => {
  test("publishing is tag-only while normal CI remains enabled", () => {
    expect(existsSync(workflow("preview"))).toBe(false);
    expect(release.on).toEqual({ push: { tags: ["v*"] }, workflow_dispatch: null });
    expect(ci.on.pull_request).toBeDefined();
    expect(ci.on.push.branches).toEqual(["main", "rebrand"]);
  });

  test("release builds every asset, validates inputs, then publishes to GitHub Releases", () => {
    expect(Object.keys(release.jobs)).toEqual(["build", "validate-release-inputs", "release"]);
    expect(release.jobs.release.needs).toEqual(["build", "validate-release-inputs"]);
    expect(release.jobs.build.strategy.matrix.include).toContainEqual({
      target: "x86_64-pc-windows-msvc",
      os: "windows-2022",
      name: "h0x-windows-x86_64.zip",
      libghostty_vt_optimize: "ReleaseFast",
      libghostty_vt_simd: "true",
    });
    const publish = release.jobs.release.steps.find((step: any) => step.name === "Create release");
    const files = publish.with.files.trim().split("\n").map((line: string) => line.trim());
    expect(files.slice(-2)).toEqual(["distribution/install.cmd", "dist/latest.json"]);
  });

  test("release arguments are not interpolated into executable shell text", () => {
    const input = `untrusted'\"$(echo unexpected-command)`;
    for (const args of [
      ["release-prepare", input],
      ["release-publish", input],
      ["release", input],
    ]) {
      const result = spawnSync("just", ["--dry-run", ...args], { encoding: "utf8" });
      expect(result.status).toBe(0);
      expect(result.stdout + result.stderr).not.toContain(input);
      expect(result.stdout + result.stderr).not.toContain("unexpected-command");
    }
    const result = spawnSync("just", ["--dry-run", "preview", input], { encoding: "utf8" });
    expect(result.status).not.toBe(0);
  });
});
