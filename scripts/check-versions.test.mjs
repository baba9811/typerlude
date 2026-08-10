import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { readVersions, validateVersions } from "./check-versions.mjs";

const packageNames = [
  "typeul-darwin-arm64", "typeul-darwin-x64", "typeul-linux-arm64",
  "typeul-linux-x64", "typeul-win32-arm64-msvc", "typeul-win32-x64-msvc",
];

function withFixture(change, check) {
  const root = fs.mkdtempSync(path.join(process.cwd(), ".check-versions-"));
  const version = "1.2.3";
  const optionalDependencies = Object.fromEntries(packageNames.map((name) => [name, version]));
  fs.writeFileSync(path.join(root, "Cargo.toml"), `[package]\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "Cargo.lock"), `[[package]]\nname = "typeul"\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({ version, optionalDependencies }));
  fs.mkdirSync(path.join(root, "npm"));
  for (const name of packageNames) {
    fs.mkdirSync(path.join(root, "npm", name));
    fs.writeFileSync(path.join(root, "npm", name, "package.json"), JSON.stringify({ name, version }));
  }
  try {
    change(root);
    check(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("reports every mismatched package path", () => {
  assert.throws(() => validateVersions([
    ["Cargo.toml", "1.2.3"], ["package.json", "1.2.4"],
    ["npm/typeul-linux-x64/package.json", "1.2.2"],
  ], "v1.2.3"), /package.json.*typeul-linux-x64/s);
});

test("reads one complete synchronized fixture", () => {
  withFixture(() => {}, (root) => {
    assert.equal(validateVersions(readVersions(root), "v1.2.3"), "1.2.3");
  });
});

test("rejects invalid fixture layouts", () => {
  for (const [name, change, message] of [
    ["missing manifest", (root) => fs.rmSync(path.join(root, "npm", packageNames[0]), { recursive: true }), /Native manifests/],
    ["extra manifest", (root) => fs.mkdirSync(path.join(root, "npm", "unrelated-package")), /Native manifests/],
    ["missing optional dependency", (root) => {
      const file = path.join(root, "package.json");
      const pkg = JSON.parse(fs.readFileSync(file));
      delete pkg.optionalDependencies[packageNames[0]];
      fs.writeFileSync(file, JSON.stringify(pkg));
    }, /optionalDependencies/],
    ["extra optional dependency", (root) => {
      const file = path.join(root, "package.json");
      const pkg = JSON.parse(fs.readFileSync(file));
      pkg.optionalDependencies.extra = "1.2.3";
      fs.writeFileSync(file, JSON.stringify(pkg));
    }, /optionalDependencies/],
    ["missing lock record", (root) => fs.writeFileSync(path.join(root, "Cargo.lock"), ""), /exactly one typeul package/],
    ["duplicate lock record", (root) => fs.appendFileSync(path.join(root, "Cargo.lock"), `\n[[package]]\nname = "typeul"\nversion = "1.2.3"\n`), /exactly one typeul package/],
  ]) {
    withFixture(change, (root) => assert.throws(() => readVersions(root), message), name);
  }
});

test("rejects invalid versions and tags", () => {
  for (const [version, tag] of [["^1.2.3", "v1.2.3"], ["1.2.3-beta.1", "v1.2.3"], ["1.2.3", "v1.2.4"]]) {
    withFixture((root) => {
      const file = path.join(root, "package.json");
      const pkg = JSON.parse(fs.readFileSync(file));
      pkg.version = version;
      fs.writeFileSync(file, JSON.stringify(pkg));
    }, (root) => assert.throws(() => validateVersions(readVersions(root), tag)));
  }
});

test("reports a mismatched optional dependency from a fixture", () => {
  withFixture((root) => {
    const file = path.join(root, "package.json");
    const pkg = JSON.parse(fs.readFileSync(file));
    pkg.optionalDependencies["typeul-linux-x64"] = "1.2.4";
    fs.writeFileSync(file, JSON.stringify(pkg));
  }, (root) => {
    assert.throws(
      () => validateVersions(readVersions(root), "v1.2.3"),
      /optionalDependencies\.typeul-linux-x64 \(1\.2\.4\)/,
    );
  });
});

test("uses a positional release tag", () => {
  const valid = spawnSync(process.execPath, ["scripts/check-versions.mjs", "v1.0.0"], { cwd: process.cwd(), encoding: "utf8" });
  assert.equal(valid.status, 0, valid.stderr);
  assert.equal(valid.stdout, "1.0.0\n");
  const invalid = spawnSync(process.execPath, ["scripts/check-versions.mjs", "v1.0.1"], { cwd: process.cwd(), encoding: "utf8" });
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, /Tag must be v1\.0\.0/);
});

test("ignores a branch ref when no release tag is supplied", () => {
  const result = spawnSync(process.execPath, ["scripts/check-versions.mjs"], {
    cwd: process.cwd(), encoding: "utf8", env: { ...process.env, GITHUB_REF_NAME: "main" },
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "1.0.0\n");
});
