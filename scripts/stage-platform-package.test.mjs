import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { stagePlatform } from "./stage-platform-package.mjs";
import {
  assertLicenseTreeClean, prependPath, resolveNpmCli, runNodeCli,
  validateInstalledPackageTree, validateNativeManifests, validatePackRecord,
  validatePackedManifest,
} from "./verify-package.mjs";

const sourceRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const permittedRootScripts = {
  test: "node --test tests/launcher.test.js scripts/import-tatoeba.test.mjs scripts/check-versions.test.mjs scripts/stage-platform-package.test.mjs scripts/publish-npm-packages.test.mjs && node scripts/check-versions.mjs",
  "test:launcher": "node --test tests/launcher.test.js",
  "package-check": "node scripts/verify-package.mjs",
};

function listFiles(root, prefix = "") {
  return fs.readdirSync(path.join(root, prefix), { withFileTypes: true })
    .flatMap((entry) => {
      const relative = path.join(prefix, entry.name);
      return entry.isDirectory() ? listFiles(root, relative) : [relative.split(path.sep).join("/")];
    })
    .sort();
}

function fixture(packageName = "typerlude-linux-x64") {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-stage-test-"));
  const packageDir = path.join(root, "npm", packageName);
  const executable = packageName.startsWith("typerlude-win32-") ? "typerlude.exe" : "typerlude";
  const binary = path.join(root, "target", "release", executable);
  const output = path.join(root, "staged", packageName);
  fs.mkdirSync(packageDir, { recursive: true });
  fs.mkdirSync(path.dirname(binary), { recursive: true });
  fs.copyFileSync(path.join(sourceRoot, "npm", packageName, "package.json"), path.join(packageDir, "package.json"));
  for (const name of ["LICENSE", "THIRD_PARTY_LICENSES.html", "THIRD_PARTY_NOTICES.md"]) {
    fs.copyFileSync(path.join(sourceRoot, name), path.join(root, name));
  }
  fs.cpSync(path.join(sourceRoot, "assets", "licenses"), path.join(root, "assets", "licenses"), { recursive: true });
  fs.writeFileSync(binary, "native binary");
  fs.chmodSync(binary, 0o600);
  return { root, packageDir, binary, output };
}

function withFixture(check, packageName) {
  const value = fixture(packageName);
  try {
    check(value);
  } finally {
    fs.rmSync(value.root, { recursive: true, force: true });
  }
}

function changeManifest(packageDir, change) {
  const file = path.join(packageDir, "package.json");
  const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  change(manifest);
  fs.writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
}

test("staging copies only the manifest, binary, and every committed license file", () => {
  withFixture(({ packageDir, binary, output }) => {
    stagePlatform(packageDir, binary, output);
    assert.deepEqual(listFiles(output), [
      "LICENSE",
      "THIRD_PARTY_LICENSES.html",
      "THIRD_PARTY_NOTICES.md",
      ...listFiles(path.join(sourceRoot, "assets", "licenses")).map((name) => `licenses/${name}`),
      "package.json",
      "typerlude",
    ].sort());
    assert.equal(fs.readFileSync(path.join(output, "typerlude"), "utf8"), "native binary");
    if (process.platform !== "win32") {
      assert.equal(fs.statSync(path.join(output, "typerlude")).mode & 0o777, 0o755);
    }
  });
});

test("staging accepts the unscoped Windows registry name", () => {
  withFixture(({ packageDir, binary, output }) => {
    changeManifest(packageDir, (manifest) => {
      manifest.name = "typerlude-win32-x64-msvc";
    });
    stagePlatform(packageDir, binary, output);
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(output, "package.json"))).name,
      "typerlude-win32-x64-msvc",
    );
    assert.equal(fs.readFileSync(path.join(output, "typerlude.exe"), "utf8"), "native binary");
  }, "typerlude-win32-x64-msvc");
});

test("staging requires the native manifest name, os, cpu, and files allowlist", () => {
  for (const [name, change] of [
    ["name", (manifest) => { manifest.name = "typerlude-linux-arm64"; }],
    ["os", (manifest) => { manifest.os = ["darwin"]; }],
    ["cpu", (manifest) => { manifest.cpu = ["arm64"]; }],
    ["missing file", (manifest) => { manifest.files.pop(); }],
    ["extra file", (manifest) => { manifest.files.push("src"); }],
    ["wrong binary", (manifest) => { manifest.files[0] = "../typerlude"; }],
    ["joined file", (manifest) => {
      const sorted = [...manifest.files].sort();
      manifest.files = [`${sorted[0]}\n${sorted[1]}`, ...sorted.slice(2)];
    }],
  ]) {
    withFixture(({ packageDir, binary, output }) => {
      changeManifest(packageDir, change);
      const field = ["missing file", "extra file", "wrong binary", "joined file"].includes(name) ? "files" : name;
      assert.throws(() => stagePlatform(packageDir, binary, output), new RegExp(field));
      assert.equal(fs.existsSync(output), false, `${name} created partial output`);
    });
  }
});

test("staging rejects path escapes, non-regular sources, and symlinks", () => {
  withFixture(({ root, packageDir, output }) => {
    const outside = path.join(os.tmpdir(), `typerlude-outside-${process.pid}-${Date.now()}`);
    fs.writeFileSync(outside, "outside");
    try {
      assert.throws(() => stagePlatform(packageDir, outside, output), /outside|escape|binary/);
      assert.equal(fs.existsSync(output), false);
    } finally {
      fs.rmSync(outside, { force: true });
    }
  });

  for (const [name, replace] of [
    ["manifest", ({ packageDir }) => {
      fs.rmSync(path.join(packageDir, "package.json"));
      fs.mkdirSync(path.join(packageDir, "package.json"));
    }],
    ["binary", ({ binary }) => {
      fs.rmSync(binary);
      fs.mkdirSync(binary);
    }],
    ["license", ({ root }) => {
      fs.rmSync(path.join(root, "LICENSE"));
      fs.mkdirSync(path.join(root, "LICENSE"));
    }],
  ]) {
    withFixture((value) => {
      replace(value);
      assert.throws(() => stagePlatform(value.packageDir, value.binary, value.output), /regular file/);
      assert.equal(fs.existsSync(value.output), false, `${name} created partial output`);
    });
  }

  if (process.platform !== "win32") {
    for (const [name, replace] of [
      ["manifest", ({ root, packageDir }) => {
        fs.renameSync(path.join(packageDir, "package.json"), path.join(root, "manifest.json"));
        fs.symlinkSync(path.join(root, "manifest.json"), path.join(packageDir, "package.json"));
      }],
      ["binary", ({ root, binary }) => {
        fs.renameSync(binary, path.join(root, "real-binary"));
        fs.symlinkSync(path.join(root, "real-binary"), binary);
      }],
      ["license", ({ root }) => {
        fs.symlinkSync(path.join(root, "LICENSE"), path.join(root, "assets", "licenses", "linked.txt"));
      }],
    ]) {
      withFixture((value) => {
        replace(value);
        assert.throws(() => stagePlatform(value.packageDir, value.binary, value.output), /symlink|real|regular/);
        assert.equal(fs.existsSync(value.output), false, `${name} symlink created partial output`);
      });
    }

    withFixture((value) => {
      const outside = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-license-escape-"));
      try {
        fs.renameSync(path.join(value.root, "assets", "licenses"), path.join(outside, "licenses"));
        fs.rmSync(path.join(value.root, "assets"), { recursive: true });
        fs.symlinkSync(outside, path.join(value.root, "assets"));
        assert.throws(() => stagePlatform(value.packageDir, value.binary, value.output), /outside/);
        assert.equal(fs.existsSync(value.output), false);
      } finally {
        fs.rmSync(outside, { recursive: true, force: true });
      }
    });
  }
});

test("staging accepts only an absent or empty real output directory", () => {
  withFixture(({ packageDir, binary, output }) => {
    fs.mkdirSync(output, { recursive: true });
    fs.writeFileSync(path.join(output, "keep.txt"), "keep");
    assert.throws(() => stagePlatform(packageDir, binary, output), /empty/);
    assert.equal(fs.readFileSync(path.join(output, "keep.txt"), "utf8"), "keep");
  });

  if (process.platform !== "win32") {
    withFixture(({ root, packageDir, binary, output }) => {
      const target = path.join(root, "staged", "target");
      fs.mkdirSync(target, { recursive: true });
      fs.symlinkSync(target, output);
      assert.throws(() => stagePlatform(packageDir, binary, output), /symlink|real directory/);
    });
    withFixture(({ root, packageDir, binary, output }) => {
      fs.mkdirSync(path.dirname(output), { recursive: true });
      fs.symlinkSync(path.join(root, "missing-output-target"), output);
      assert.throws(() => stagePlatform(packageDir, binary, output), /symlink|real directory/);
    });
  }
});

test("pack records require the exact versioned file and mode allowlist", () => {
  const expected = {
    name: "typerlude",
    version: "1.0.0",
    files: new Map([["package.json", 0o644], ["bin/typerlude.js", 0o755]]),
  };
  const record = {
    name: "typerlude",
    version: "1.0.0",
    filename: "typerlude-1.0.0.tgz",
    files: [
      { path: "bin/typerlude.js", mode: 0o755, size: 10 },
      { path: "package.json", mode: 0o644, size: 20 },
    ],
    entryCount: 2,
    bundled: [],
  };
  assert.equal(validatePackRecord(record, expected), "typerlude-1.0.0.tgz");

  const scoped = structuredClone(record);
  scoped.name = "typerlude-win32-x64-msvc";
  scoped.filename = "typerlude-win32-x64-msvc-1.0.0.tgz";
  assert.equal(
    validatePackRecord(scoped, { ...expected, name: scoped.name }),
    scoped.filename,
  );

  for (const [name, change, message] of [
    ["version", (value) => { value.version = "1.0.1"; }, /version/],
    ["missing", (value) => { value.files.pop(); value.entryCount -= 1; }, /files/],
    ["raw source", (value) => { value.files.push({ path: "src/main.rs", mode: 0o644, size: 1 }); value.entryCount += 1; }, /files/],
    ["mode", (value) => { value.files[0].mode = 0o644; }, /mode/],
    ["bundled", (value) => { value.bundled = ["raw-source"]; }, /bundled/],
  ]) {
    const mutated = structuredClone(record);
    change(mutated);
    assert.throws(() => validatePackRecord(mutated, expected), message, name);
  }
});

test("packed manifests require the exact root scripts and no native scripts", () => {
  const manifest = {
    name: "typerlude",
    version: "1.0.0",
    files: ["bin/typerlude.js", "LICENSE"],
    scripts: permittedRootScripts,
  };
  validatePackedManifest(manifest, {
    name: "typerlude",
    version: "1.0.0",
    files: ["bin/typerlude.js", "LICENSE"],
    scripts: permittedRootScripts,
  });
  for (const [change, message] of [
    [(value) => { value.private = true; }, /private/],
    [(value) => { value.scripts.preversion = "node lifecycle.js"; }, /scripts/],
    [(value) => { delete value.scripts["test:launcher"]; }, /scripts/],
    [(value) => { value.files.push("src"); }, /files/],
    [(value) => { value.files = ["bin/typerlude.js\nLICENSE"]; }, /files/],
  ]) {
    const mutated = structuredClone(manifest);
    change(mutated);
    assert.throws(() => validatePackedManifest(mutated, {
      name: "typerlude",
      version: "1.0.0",
      files: ["bin/typerlude.js", "LICENSE"],
      scripts: permittedRootScripts,
    }), message);
  }

  const native = { name: "typerlude-linux-x64", version: "1.0.0", files: ["typerlude"] };
  validatePackedManifest(native, { ...native, scripts: null });
  native.scripts = { test: "node --test" };
  assert.throws(() => validatePackedManifest(native, {
    name: native.name,
    version: native.version,
    files: native.files,
    scripts: null,
  }), /scripts/);
});

test("packed manifests pin platform selectors and the root native dependency closure", () => {
  const native = {
    name: "typerlude-win32-x64-msvc",
    version: "1.0.0",
    files: ["typerlude.exe"],
    os: ["win32"],
    cpu: ["x64"],
  };
  validatePackedManifest(native, {
    name: native.name, version: native.version, files: native.files, os: "win32", cpu: "x64",
    bin: null, dependencies: {}, optionalDependencies: {}, peerDependencies: {}, peerDependenciesMeta: {},
  });
  for (const field of ["os", "cpu"]) {
    const mutated = structuredClone(native);
    mutated[field] = [field === "os" ? "linux" : "arm64"];
    assert.throws(() => validatePackedManifest(mutated, {
      name: native.name, version: native.version, files: native.files, os: "win32", cpu: "x64",
      bin: null, dependencies: {}, optionalDependencies: {}, peerDependencies: {}, peerDependenciesMeta: {},
    }), new RegExp(field));
  }
  for (const [field, value] of [
    ["bin", { typerlude: "typerlude.exe" }],
    ["dependencies", { extra: "1.0.0" }],
    ["optionalDependencies", { extra: "1.0.0" }],
    ["peerDependencies", { extra: "1.0.0" }],
    ["peerDependenciesMeta", { extra: { optional: true } }],
  ]) {
    const mutated = structuredClone(native);
    mutated[field] = value;
    assert.throws(() => validatePackedManifest(mutated, {
      name: native.name, version: native.version, files: native.files, os: "win32", cpu: "x64",
      bin: null, dependencies: {}, optionalDependencies: {}, peerDependencies: {}, peerDependenciesMeta: {},
    }), new RegExp(field));
  }

  const dependencies = { "typerlude-linux-x64": "1.0.0", "typerlude-linux-arm64": "1.0.0" };
  const root = {
    name: "typerlude", version: "1.0.0", files: ["bin/typerlude.js"],
    bin: { typerlude: "bin/typerlude.js" }, optionalDependencies: dependencies,
  };
  validatePackedManifest(root, {
    name: root.name, version: root.version, files: root.files, bin: root.bin,
    dependencies: {}, optionalDependencies: dependencies, peerDependencies: {}, peerDependenciesMeta: {},
  });
  for (const change of [
    (value) => { delete value.optionalDependencies["typerlude-linux-x64"]; },
    (value) => { value.optionalDependencies.extra = "1.0.0"; },
    (value) => { value.optionalDependencies["typerlude-linux-x64"] = "1.0.1"; },
  ]) {
    const mutated = structuredClone(root);
    change(mutated);
    assert.throws(() => validatePackedManifest(mutated, {
      name: root.name, version: root.version, files: root.files, bin: root.bin,
      dependencies: {}, optionalDependencies: dependencies, peerDependencies: {}, peerDependenciesMeta: {},
    }), /optionalDependencies/);
  }
  for (const change of [
    (value) => { value.bin.typerlude = "src/main.js"; },
    (value) => { value.dependencies = { extra: "1.0.0" }; },
    (value) => { value.peerDependencies = { extra: "1.0.0" }; },
    (value) => { value.peerDependenciesMeta = { extra: { optional: true } }; },
  ]) {
    const mutated = structuredClone(root);
    change(mutated);
    assert.throws(() => validatePackedManifest(mutated, {
      name: root.name, version: root.version, files: root.files, bin: root.bin,
      dependencies: {}, optionalDependencies: dependencies, peerDependencies: {}, peerDependenciesMeta: {},
    }), /bin|[Dd]ependencies/);
  }
});

test("fake npm prepends the existing Windows Path spelling without a duplicate PATH", () => {
  assert.deepEqual(prependPath({ Path: "real-bin", HOME: "home" }, "fake-bin", "win32"), {
    Path: "fake-bin;real-bin",
    HOME: "home",
  });
});

test("Node CLI execution preserves shell metacharacters as exact arguments", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude cli &^()%! "));
  try {
    const cli = path.join(root, "npm cli &^()%!.mjs");
    fs.writeFileSync(cli, "process.stdout.write(JSON.stringify(process.argv.slice(2)));\n");
    const args = [
      path.join(root, "package & root.tgz"),
      "literal|pipe", "redirect<input", "redirect>output", "caret^value",
      "percent%value", "bang!value", "(parentheses)",
    ];
    assert.deepEqual(JSON.parse(runNodeCli(cli, args)), args);

    const npmDir = path.join(root, "node_modules", "npm", "bin");
    fs.mkdirSync(npmDir, { recursive: true });
    const npmCli = path.join(npmDir, "npm-cli.js");
    fs.writeFileSync(npmCli, "// fixture\n");
    assert.equal(resolveNpmCli({ npm_execpath: npmCli }, path.join(root, "node.exe"), "win32"), fs.realpathSync(npmCli));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("installed package trees reject extra, missing, replaced, and linked legal files", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-installed-tree-"));
  try {
    const source = path.join(root, "source-LICENSE");
    const installed = path.join(root, "installed");
    fs.mkdirSync(installed);
    fs.writeFileSync(source, "license bytes\n");
    fs.writeFileSync(path.join(installed, "package.json"), "{}\n");
    fs.writeFileSync(path.join(installed, "LICENSE"), "license bytes\n");
    const expected = ["LICENSE", "package.json"];
    const copies = new Map([["LICENSE", source]]);
    validateInstalledPackageTree(installed, expected, copies);

    fs.writeFileSync(path.join(installed, ".private"), "not allowed");
    assert.throws(() => validateInstalledPackageTree(installed, expected, copies), /files|private/);
    fs.rmSync(path.join(installed, ".private"));

    fs.rmSync(path.join(installed, "LICENSE"));
    assert.throws(() => validateInstalledPackageTree(installed, expected, copies), /files|LICENSE/);
    fs.writeFileSync(path.join(installed, "LICENSE"), "");
    assert.throws(() => validateInstalledPackageTree(installed, expected, copies), /LICENSE.*source|differs/);

    if (process.platform !== "win32") {
      fs.rmSync(path.join(installed, "LICENSE"));
      fs.symlinkSync(source, path.join(installed, "LICENSE"));
      assert.throws(() => validateInstalledPackageTree(installed, expected, copies), /symlink|regular/);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("all source native manifests are validated before selecting the host", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-native-manifests-"));
  const version = JSON.parse(fs.readFileSync(path.join(sourceRoot, "package.json"))).version;
  try {
    fs.cpSync(path.join(sourceRoot, "npm"), path.join(root, "npm"), { recursive: true });
    validateNativeManifests(root, version);
    const host = process.platform === "win32"
      ? `typerlude-${process.platform}-${process.arch}-msvc`
      : `typerlude-${process.platform}-${process.arch}`;
    const nonHost = fs.readdirSync(path.join(root, "npm")).find((name) => name !== host);
    changeManifest(path.join(root, "npm", nonHost), (manifest) => {
      manifest.dependencies = { "private-registry-code": "1.0.0" };
    });
    assert.throws(() => validateNativeManifests(root, version), new RegExp(`${nonHost}.*dependencies|dependencies.*${nonHost}`, "s"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("CLI duplicate flags reject an empty first value", () => {
  const result = spawnSync(process.execPath, [
    path.join(sourceRoot, "scripts", "stage-platform-package.mjs"),
    "--package-dir", "", "--package-dir", "second",
    "--binary", "binary", "--out", "out",
  ], { encoding: "utf8" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /usage:/);
});

test("license cleanliness rejects tracked changes and all untracked license files", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-license-git-"));
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
  };
  try {
    git("init", "--quiet");
    fs.mkdirSync(path.join(root, "assets", "licenses"), { recursive: true });
    fs.writeFileSync(path.join(root, "THIRD_PARTY_LICENSES.html"), "generated\n");
    fs.writeFileSync(path.join(root, "assets", "licenses", "known.txt"), "known\n");
    git("add", ".");
    git("-c", "user.name=Typerlude Test", "-c", "user.email=test@example.invalid", "commit", "--quiet", "-m", "fixture");
    assertLicenseTreeClean(root);

    fs.writeFileSync(path.join(root, "THIRD_PARTY_LICENSES.html"), "stale\n");
    assert.throws(() => assertLicenseTreeClean(root), /generated license files differ/);
    fs.writeFileSync(path.join(root, "THIRD_PARTY_LICENSES.html"), "generated\n");
    fs.writeFileSync(path.join(root, "assets", "licenses", "untracked.txt"), "new\n");
    assert.throws(() => assertLicenseTreeClean(root), /untracked license files.*untracked\.txt/s);
    fs.rmSync(path.join(root, "assets", "licenses", "untracked.txt"));

    fs.appendFileSync(path.join(root, ".git", "info", "exclude"), "assets/licenses/ignored-private.txt\n");
    fs.writeFileSync(path.join(root, "assets", "licenses", "ignored-private.txt"), "private\n");
    assert.throws(() => assertLicenseTreeClean(root), /untracked license files.*ignored-private\.txt/s);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("license cleanliness skips an archive without its own Git entry", () => {
  const parent = fs.mkdtempSync(path.join(os.tmpdir(), "typerlude-license-parent-git-"));
  try {
    const result = spawnSync("git", ["init", "--quiet"], { cwd: parent, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    const archive = path.join(parent, "source-archive");
    fs.mkdirSync(path.join(archive, "assets", "licenses"), { recursive: true });
    fs.writeFileSync(path.join(archive, "THIRD_PARTY_LICENSES.html"), "generated\n");
    fs.writeFileSync(path.join(archive, "assets", "licenses", "archive.txt"), "license\n");
    assert.doesNotThrow(() => assertLicenseTreeClean(archive));
  } finally {
    fs.rmSync(parent, { recursive: true, force: true });
  }
});
