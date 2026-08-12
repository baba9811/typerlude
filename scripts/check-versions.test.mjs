import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { readVersions, validateVersions } from "./check-versions.mjs";

const nativePackages = [
  ["typerlude-darwin-arm64", "typerlude-darwin-arm64"],
  ["typerlude-darwin-x64", "typerlude-darwin-x64"],
  ["typerlude-linux-arm64", "typerlude-linux-arm64"],
  ["typerlude-linux-x64", "typerlude-linux-x64"],
  ["typerlude-win32-arm64-msvc", "typerlude-win32-arm64-msvc"],
  ["typerlude-win32-x64-msvc", "typerlude-win32-x64-msvc"],
];
const packageNames = nativePackages.map(([, name]) => name);
const packageDirectories = nativePackages.map(([directory]) => directory);

function writeVersionFixture(root) {
  const version = "1.2.3";
  const optionalDependencies = Object.fromEntries(packageNames.map((name) => [name, version]));
  fs.writeFileSync(path.join(root, "Cargo.toml"), `[package]\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "Cargo.lock"), `[[package]]\nname = "typerlude"\nversion = "${version}"\n`);
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({
    name: "typerlude", version, optionalDependencies,
  }));
  fs.mkdirSync(path.join(root, "npm"));
  for (const [directory, name] of nativePackages) {
    fs.mkdirSync(path.join(root, "npm", directory));
    fs.writeFileSync(path.join(root, "npm", directory, "package.json"), JSON.stringify({ name, version }));
  }
}

function withFixture(change, check) {
  const root = fs.mkdtempSync(path.join(process.cwd(), ".check-versions-"));
  writeVersionFixture(root);
  try {
    change(root);
    check(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function namedWorkflowStep(workflow, name) {
  const marker = `      - name: ${name}\n`;
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1, `missing workflow step: ${name}`);
  const rest = workflow.slice(start + marker.length);
  const boundary = rest.search(/\n(?:      - |  [a-z][a-z0-9-]*:)/);
  const end = boundary === -1 ? workflow.length : start + marker.length + boundary;
  return { start, end, text: workflow.slice(start, end) };
}

function replaceWorkflowStep(workflow, name, transform) {
  const { start, end, text } = namedWorkflowStep(workflow, name);
  return workflow.slice(0, start) + transform(text) + workflow.slice(end);
}

function replaceWorkflowJob(workflow, name, transform) {
  const marker = `  ${name}:\n`;
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1, `missing workflow job: ${name}`);
  const rest = workflow.slice(start + marker.length);
  const boundary = rest.search(/\n {2}[a-z][a-z0-9-]*:\n/);
  const end = boundary === -1 ? workflow.length : start + marker.length + boundary;
  return workflow.slice(0, start) + transform(workflow.slice(start, end)) + workflow.slice(end);
}

function assertStepCondition(step, name, condition) {
  assert.equal(
    step.text.match(/^ {8}if: (.+)$/m)?.[1],
    condition,
    `${name} must use its exact registry condition`,
  );
}

function assertReleaseSourcePolicy(workflow) {
  const validation = namedWorkflowStep(workflow, "Validate tag and ancestry");
  assert.match(
    validation.text,
    /^ {10}\[\[ "\$GITHUB_EVENT_NAME" != "workflow_dispatch" \|\| "\$tag_commit" == "\$GITHUB_SHA" \]\] \|\| \{$/m,
    "manual dispatch must bind the selected ref commit to the release tag commit",
  );

  const checkoutSteps = [...workflow.matchAll(
    /^ {6}- uses: actions\/checkout@[^\n]+(?:\n(?! {6}- )[^\n]*)*/gm,
  )].map((match) => match[0]);
  assert.equal(checkoutSteps.length, 5, "release workflow must retain five source checkouts");
  for (const step of checkoutSteps) {
    assert.equal(
      step.match(/^ {10}ref: (.+)$/m)?.[1],
      "${{ env.RELEASE_TAG }}",
      "every release source checkout must use RELEASE_TAG",
    );
  }
}

function assertRegistryReleasePolicy(workflow) {
  assert.doesNotMatch(
    workflow,
    /TYPERLUDE_REGISTRY_BOOTSTRAP|secrets\.(?:CRATES_TOKEN|NPM_TOKEN)|NODE_AUTH_TOKEN/,
    "release workflow must not contain bootstrap switches or long-lived registry credentials",
  );

  for (const jobName of ["publish-cargo", "publish-npm"]) {
    const marker = `  ${jobName}:\n`;
    const start = workflow.indexOf(marker);
    assert.notEqual(start, -1, `missing workflow job: ${jobName}`);
    const rest = workflow.slice(start + marker.length);
    const boundary = rest.search(/\n {2}[a-z][a-z0-9-]*:\n/);
    const job = workflow.slice(start, boundary === -1 ? workflow.length : start + marker.length + boundary);
    assert.doesNotMatch(
      job,
      /secrets\.|BOOTSTRAP|(?:_authToken|auth-type|npm config set)/i,
      `${jobName} must not contain a long-lived credential fallback`,
    );
    assert.match(job, /^ {4}environment: release$/m, `${jobName} must use the release environment`);
    assert.match(job, /^ {6}id-token: write$/m, `${jobName} must request an OIDC token`);
  }

  const condition = "steps.cargo-version.outputs.publish == 'true'";
  const cargoAuth = namedWorkflowStep(workflow, "Authenticate with crates.io");
  const cargoCredential = namedWorkflowStep(workflow, "Require OIDC credential");
  const cargoPublish = namedWorkflowStep(workflow, "Publish Cargo with OIDC");
  assertStepCondition(cargoAuth, "Authenticate with crates.io", condition);
  assertStepCondition(cargoCredential, "Require OIDC credential", condition);
  assertStepCondition(cargoPublish, "Publish Cargo with OIDC", condition);
  assert.match(cargoAuth.text, /rust-lang\/crates-io-auth-action@/);
  assert.match(cargoCredential.text, /CARGO_REGISTRY_TOKEN: \$\{\{ steps\.crates-auth\.outputs\.token \}\}/);
  assert.match(cargoPublish.text, /^ {8}run: cargo publish --locked$/m);
  assert.match(cargoPublish.text, /CARGO_REGISTRY_TOKEN: \$\{\{ steps\.crates-auth\.outputs\.token \}\}/);
  assert.ok(cargoAuth.start < cargoCredential.start && cargoCredential.start < cargoPublish.start);

  const npmOidc = namedWorkflowStep(workflow, "Publish npm with OIDC, native packages first");
  assert.equal(npmOidc.text.match(/^ {8}if:/m), null, "npm OIDC publication must be unconditional");
  assert.match(npmOidc.text, /node scripts\/publish-npm-packages\.mjs "\$version"/);
  assert.match(workflow, /publish-npm:\n\s+needs: publish-cargo/);
}

function assertSteadyStateReleaseDocumentation(markdown) {
  assert.match(markdown, /`baba9811\/typerlude`/);
  assert.match(markdown, /workflow filename: `release\.yml`/i);
  assert.match(markdown, /environment: `release`/i);
  assert.match(markdown, /`make release`/);
  assert.match(markdown, /signed annotated tag/i);
  assert.match(markdown, /OIDC-only/i);
  assert.match(markdown, /immutable releases.*only.*after.*enabled/is);
  for (const name of ["typerlude", ...packageNames]) {
    assert.ok(markdown.includes(`\`${name}\``), `missing trusted npm package: ${name}`);
  }
  assert.doesNotMatch(
    markdown,
    /TYPERLUDE_REGISTRY_BOOTSTRAP|CRATES_TOKEN|NPM_TOKEN|NODE_AUTH_TOKEN|\.env|bootstrap|fallback[ -]token/i,
    "steady-state release documentation must not retain token bootstrap instructions",
  );
}

test("reports every mismatched package path", () => {
  assert.throws(() => validateVersions([
    ["Cargo.toml", "1.2.3"], ["package.json", "1.2.4"],
    ["npm/typerlude-linux-x64/package.json", "1.2.2"],
  ], "v1.2.3"), /package.json.*typerlude-linux-x64/s);
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
      pkg.name = "not-typerlude";
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
    ["missing lock record", (root) => fs.writeFileSync(path.join(root, "Cargo.lock"), ""), /exactly one typerlude package/],
    ["duplicate lock record", (root) => fs.appendFileSync(path.join(root, "Cargo.lock"), `\n[[package]]\nname = "typerlude"\nversion = "1.2.3"\n`), /exactly one typerlude package/],
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
    pkg.optionalDependencies["typerlude-linux-x64"] = "1.2.4";
    fs.writeFileSync(file, JSON.stringify(pkg));
  }, (root) => {
    assert.throws(
      () => validateVersions(readVersions(root), "v1.2.3"),
      /optionalDependencies\.typerlude-linux-x64 \(1\.2\.4\)/,
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

test("direct execution guard compares resolved platform file URLs", async () => {
  const module = await import("./check-versions.mjs");
  const entry = path.join("directory with spaces", "check versions.mjs");
  const entryUrl = pathToFileURL(path.resolve(entry)).href;
  assert.equal(module.isDirectExecution(entryUrl, entry), true);
  assert.equal(module.isDirectExecution(entryUrl, `${entry}.other`), false);
  assert.equal(module.isDirectExecution(entryUrl, undefined), false);
});

test("executes from a real path containing spaces", () => {
  const temporary = fs.mkdtempSync(path.join(process.cwd(), ".versions with spaces "));
  const root = path.join(temporary, "repository with spaces");
  try {
    fs.mkdirSync(root);
    writeVersionFixture(root);
    fs.mkdirSync(path.join(root, "scripts"));
    fs.mkdirSync(path.join(root, "bin"));
    fs.copyFileSync("scripts/check-versions.mjs", path.join(root, "scripts", "check-versions.mjs"));
    fs.copyFileSync("bin/typerlude.js", path.join(root, "bin", "typerlude.js"));

    const result = spawnSync(process.execPath, [path.join(root, "scripts", "check-versions.mjs")], {
      cwd: root,
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, "1.2.3\n");
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
});

test("CI and release use the complete Typerlude native artifact family", () => {
  const ci = fs.readFileSync(".github/workflows/ci.yml", "utf8");
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");

  for (const source of [ci, workflow]) {
    const matrixPackages = [...source.matchAll(/^\s+package: (typerlude-[^\s]+)$/gm)].map((match) => match[1]);
    assert.deepEqual(matrixPackages.sort(), [...packageNames].sort());
    assert.equal((source.match(/^\s+executable: typerlude(?:\.exe)?$/gm) ?? []).length, 6);
    assert.match(source, /archive[_Rr]oot[^\n]*typerlude-/);
    assert.match(source, /artifacts[\\/]typerlude-/);
    assert.doesNotMatch(source, new RegExp(["type", "ul"].join(""), "i"));
  }

  assert.match(fs.readFileSync("Makefile", "utf8"), /target\/release\/typerlude/);
});

test("registry publication is OIDC-only and ordered Cargo before npm", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  assertRegistryReleasePolicy(workflow);
});

test("normal registry publication remains OIDC-only", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  const cargoAuth = namedWorkflowStep(workflow, "Authenticate with crates.io");
  assert.match(cargoAuth.text, /rust-lang\/crates-io-auth-action@/);
  assert.match(workflow, /Verify Cargo package again[\s\S]*cargo package --locked[\s\S]*cargo publish --dry-run --locked/);
  assertRegistryReleasePolicy(workflow);
});

test("release source policy binds dispatch and every checkout to the release tag", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  assertReleaseSourcePolicy(workflow);

  const mismatchedDispatch = workflow.replace(
    '"$tag_commit" == "$GITHUB_SHA"',
    '"$tag_commit" == "$tag_commit"',
  );
  assert.notEqual(mismatchedDispatch, workflow, "dispatch mismatch mutation must change the workflow");
  assert.throws(
    () => assertReleaseSourcePolicy(mismatchedDispatch),
    /manual dispatch must bind/,
  );

  const wrongCheckout = replaceWorkflowJob(workflow, "publish-npm", (job) =>
    job.replace("ref: ${{ env.RELEASE_TAG }}", "ref: ${{ github.sha }}"));
  assert.notEqual(wrongCheckout, workflow, "checkout mutation must change the workflow");
  assert.throws(
    () => assertReleaseSourcePolicy(wrongCheckout),
    /every release source checkout must use RELEASE_TAG/,
  );
});

test("release documentation describes only the trusted-publishing steady state", () => {
  assertSteadyStateReleaseDocumentation(fs.readFileSync("docs/releasing.md", "utf8"));
});

test("registry security assertions reject unsafe workflow mutations", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  const mutations = [
    [
      "long-lived Cargo secret in OIDC publication",
      replaceWorkflowStep(workflow, "Publish Cargo with OIDC", (step) =>
        step.replace("steps.crates-auth.outputs.token", "secrets.CRATES_TOKEN")),
      /must not contain bootstrap switches or long-lived registry credentials/,
    ],
    [
      "long-lived npm secret in OIDC publication",
      replaceWorkflowStep(workflow, "Publish npm with OIDC, native packages first", (step) =>
        step.replace("        shell: bash\n", "        env:\n          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}\n        shell: bash\n")),
      /must not contain bootstrap switches or long-lived registry credentials/,
    ],
    [
      "generic npm token fallback",
      replaceWorkflowStep(workflow, "Publish npm with OIDC, native packages first", (step) =>
        step.replace(
          "        shell: bash\n",
          "        env:\n          TOKEN: ${{ secrets.REGISTRY_PUBLISH_TOKEN }}\n        shell: bash\n",
        ).replace(
          "          set -euo pipefail\n",
          "          set -euo pipefail\n          npm config set //registry.npmjs.org/:_authToken \"$TOKEN\"\n",
        )),
      /publish-npm must not contain a long-lived credential fallback/,
    ],
    [
      "missing Cargo OIDC permission",
      replaceWorkflowJob(workflow, "publish-cargo", (job) => job.replace("id-token: write", "id-token: none")),
      /publish-cargo must request an OIDC token/,
    ],
    [
      "conditional npm publication fallback",
      replaceWorkflowStep(workflow, "Publish npm with OIDC, native packages first", (step) =>
        step.replace("        shell: bash\n", "        if: vars.USE_FALLBACK == '1'\n        shell: bash\n")),
      /npm OIDC publication must be unconditional/,
    ],
  ];

  for (const [name, mutated, message] of mutations) {
    assert.throws(() => assertRegistryReleasePolicy(mutated), message, name);
  }
});

test("release requires a verified signed tag", () => {
  const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");
  const release = fs.readFileSync("scripts/release.sh", "utf8");
  assert.match(workflow, /\.object\.type == "tag"/);
  assert.match(workflow, /\.verification\.verified == true/);
  assert.match(release, /git tag -s /);
  assert.match(release, /git tag -s -m "Typerlude v\$version" "v\$version"/);
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
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-release-test-"));
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
  fs.writeFileSync(path.join(root, "Cargo.toml"), "[package]\nname = \"typerlude\"\nversion = \"1.0.0\"\n");
  fs.writeFileSync(path.join(root, "Cargo.lock"), "version = 3\n");
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify({
    name: "typerlude",
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
