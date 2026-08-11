import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { readVersions, validateVersions } from "./check-versions.mjs";

const nativePackages = [
  ["typeul-darwin-arm64", "typeul-darwin-arm64"],
  ["typeul-darwin-x64", "typeul-darwin-x64"],
  ["typeul-linux-arm64", "typeul-linux-arm64"],
  ["typeul-linux-x64", "typeul-linux-x64"],
  ["typeul-win32-arm64-msvc", "@baba9811/typeul-win32-arm64-msvc"],
  ["typeul-win32-x64-msvc", "@baba9811/typeul-win32-x64-msvc"],
];
const packageNames = nativePackages.map(([, name]) => name);
const packageDirectories = nativePackages.map(([directory]) => directory);

function withFixture(change, check) {
  const root = fs.mkdtempSync(path.join(process.cwd(), ".check-versions-"));
  const version = "1.2.3";
  const optionalDependencies = Object.fromEntries(packageNames.map((name) => [name, version]));
  fs.writeFileSync(path.join(root, "Cargo.toml"), `[package]\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "Cargo.lock"), `[[package]]\nname = "typeul"\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({
    name: "@baba9811/typeul", version, optionalDependencies,
  }));
  fs.mkdirSync(path.join(root, "npm"));
  for (const [directory, name] of nativePackages) {
    fs.mkdirSync(path.join(root, "npm", directory));
    fs.writeFileSync(path.join(root, "npm", directory, "package.json"), JSON.stringify({ name, version }));
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
    ["wrong root package name", (root) => {
      const file = path.join(root, "package.json");
      const pkg = JSON.parse(fs.readFileSync(file));
      pkg.name = "typeul";
      fs.writeFileSync(file, JSON.stringify(pkg));
    }, /root npm package/],
    ["missing manifest", (root) => fs.rmSync(path.join(root, "npm", packageDirectories[0]), { recursive: true }), /Native manifests/],
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
  const version = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
  const [major, minor, patch] = version.split(".").map(Number);
  const valid = spawnSync(process.execPath, ["scripts/check-versions.mjs", `v${version}`], { cwd: process.cwd(), encoding: "utf8" });
  assert.equal(valid.status, 0, valid.stderr);
  assert.equal(valid.stdout, `${version}\n`);
  const invalid = spawnSync(process.execPath, ["scripts/check-versions.mjs", `v${major}.${minor}.${patch + 1}`], { cwd: process.cwd(), encoding: "utf8" });
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, new RegExp(`Tag must be v${version.replaceAll(".", "\\.")}`));
});

test("ignores a branch ref when no release tag is supplied", () => {
  const result = spawnSync(process.execPath, ["scripts/check-versions.mjs"], {
    cwd: process.cwd(), encoding: "utf8", env: { ...process.env, GITHUB_REF_NAME: "main" },
  });
  assert.equal(result.status, 0, result.stderr);
  const version = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
  assert.equal(result.stdout, `${version}\n`);
});

test("release uses Cargo OIDC independently from npm bootstrap", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  assert.match(workflow, /rust-lang\/crates-io-auth-action@/);
  assert.match(workflow, /Verify Cargo package again[\s\S]*cargo package --locked[\s\S]*cargo publish --dry-run --locked/);
  assert.doesNotMatch(workflow, /Publish Cargo bootstrap|secrets\.CRATES_TOKEN|TYPEUL_BOOTSTRAP/);
  assert.match(workflow, /TYPEUL_NPM_BOOTSTRAP/);
  assert.match(workflow, /Publish npm bootstrap, native packages first/);
});

test("release requires a verified signed tag", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  const release = fs.readFileSync("scripts/release.sh", "utf8");
  assert.match(workflow, /\.object\.type == "tag"/);
  assert.match(workflow, /\.verification\.verified == true/);
  assert.match(release, /git tag -s /);
});

test("Make release rejects command-line VERSION without evaluating Make functions", () => {
  const interactive = spawnSync("make", ["--no-print-directory", "-n", "release"], { encoding: "utf8" });
  assert.equal(interactive.status, 0, interactive.stderr);
  assert.equal(interactive.stdout, "scripts/release.sh\n");

  const result = spawnSync("make", [
    "-n", "release", "VERSION=$(shell printf MAKE_FUNCTION_EXECUTED >&2)",
  ], {
    encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  assert.doesNotMatch(`${result.stdout}${result.stderr}`, /MAKE_FUNCTION_EXECUTED/);
  assert.match(result.stderr, /VERSION=1\.2\.3 make release/);

  const bypass = spawnSync("make", [
    "-f", "Makefile", "-f", "-", "guard-probe", "MAKECMDGOALS=not-release",
    "VERSION=$(shell printf MAKE_GUARD_BYPASSED >&2)",
  ], {
    encoding: "utf8",
    input: "$(VERSION)\nguard-probe:\n\t@:\n",
  });
  assert.notEqual(bypass.status, 0);
  assert.doesNotMatch(`${bypass.stdout}${bypass.stderr}`, /MAKE_GUARD_BYPASSED/);
});

function run(root, command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", ...options });
  assert.equal(result.status, 0, `${command} ${args.join(" ")}\n${result.stderr}`);
  return result.stdout.trim();
}

function releaseFailureFixture(failure) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "typeul-release-test-"));
  const remote = path.join(temporary, "origin.git");
  const root = path.join(temporary, "repo");
  const fakeBin = path.join(temporary, "bin");
  fs.mkdirSync(root);
  fs.mkdirSync(fakeBin);
  run(temporary, "git", ["init", "--bare", remote]);
  run(root, "git", ["init", "-b", "main"]);
  run(root, "git", ["config", "user.name", "Release Test"]);
  run(root, "git", ["config", "user.email", "release@example.com"]);
  const signingKey = path.join(temporary, "release-signing-key");
  run(temporary, "ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-C", "release test", "-f", signingKey]);
  run(root, "git", ["config", "gpg.format", "ssh"]);
  run(root, "git", ["config", "user.signingkey", `${signingKey}.pub`]);

  fs.mkdirSync(path.join(root, "scripts"));
  fs.copyFileSync("scripts/release.sh", path.join(root, "scripts/release.sh"));
  fs.chmodSync(path.join(root, "scripts/release.sh"), 0o755);
  fs.writeFileSync(path.join(root, "scripts/check-versions.mjs"), `
    import fs from "node:fs";
    const version = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
    if (process.argv[2] && process.argv[2] !== \`v\${version}\`) process.exit(1);
    console.log(version);
  `);
  fs.writeFileSync(path.join(root, "scripts/verify-package.mjs"), "");
  fs.writeFileSync(path.join(root, "Cargo.toml"), "[package]\nname = \"typeul\"\nversion = \"1.0.0\"\n");
  fs.writeFileSync(path.join(root, "Cargo.lock"), "version = 3\n");
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({
    name: "@baba9811/typeul",
    version: "1.0.0",
    optionalDependencies: Object.fromEntries(packageNames.map((name) => [name, "1.0.0"])),
  }, null, 2));
  fs.mkdirSync(path.join(root, "npm"));
  for (const [directory, name] of nativePackages) {
    fs.mkdirSync(path.join(root, "npm", directory));
    fs.writeFileSync(path.join(root, "npm", directory, "package.json"), `${JSON.stringify({ name, version: "1.0.0" }, null, 2)}\n`);
  }
  fs.writeFileSync(path.join(root, "THIRD_PARTY_LICENSES.html"), "original\n");

  fs.writeFileSync(path.join(fakeBin, "make"), "#!/bin/sh\n[ \"${1-}\" = test ] && [ \"${2-}\" = licenses ] && printf 'generated\\n' > THIRD_PARTY_LICENSES.html\nexit 0\n");
  for (const name of ["cargo", "npm", "go"]) {
    fs.writeFileSync(path.join(fakeBin, name), "#!/bin/sh\nexit 0\n");
  }
  for (const name of ["make", "cargo", "npm", "go"]) fs.chmodSync(path.join(fakeBin, name), 0o755);

  run(root, "git", ["add", "."]);
  run(root, "git", ["commit", "-m", "initial"]);
  const base = run(root, "git", ["rev-parse", "HEAD"]);
  run(root, "git", ["remote", "add", "origin", remote]);
  run(root, "git", ["push", "-u", "origin", "main"]);

  if (failure === "commit") {
    const hook = path.join(root, ".git/hooks/pre-commit");
    fs.writeFileSync(hook, "#!/bin/sh\nexit 1\n");
    fs.chmodSync(hook, 0o755);
  } else {
    const hook = path.join(remote, "hooks/pre-receive");
    fs.writeFileSync(hook, "#!/bin/sh\nexit 1\n");
    fs.chmodSync(hook, 0o755);
  }

  const result = spawnSync("bash", ["scripts/release.sh", "1.0.1"], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, VERSION: "", PATH: `${fakeBin}${path.delimiter}${process.env.PATH}` },
  });
  try {
    assert.notEqual(result.status, 0, "the injected failure must stop the release");
    assert.match(result.stdout, /Current version: 1\.0\.0/);
    assert.match(result.stdout, /Current tag: \(none\)/);
    assert.match(result.stdout, /Releasing v1\.0\.1/);
    assert.equal(run(root, "git", ["rev-parse", "HEAD"]), base);
    assert.equal(run(root, "git", ["status", "--porcelain"]), "");
    assert.equal(run(root, "git", ["tag", "--list"]), "");
    assert.equal(JSON.parse(fs.readFileSync(path.join(root, "package.json"))).version, "1.0.0");
    assert.equal(fs.readFileSync(path.join(root, "THIRD_PARTY_LICENSES.html"), "utf8"), "original\n");
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

test("release restores index, worktree, and generated licenses after commit failure", () => {
  releaseFailureFixture("commit");
});

test("release removes its commit and tag after atomic push failure", () => {
  releaseFailureFixture("push");
});
