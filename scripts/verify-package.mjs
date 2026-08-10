import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { readVersions, validateVersions } from "./check-versions.mjs";
import { stagePlatform } from "./stage-platform-package.mjs";

const { packageFor } = createRequire(import.meta.url)("../bin/typeul.js");
const supportedPairs = [
  ["darwin", "arm64"], ["darwin", "x64"], ["linux", "arm64"],
  ["linux", "x64"], ["win32", "arm64"], ["win32", "x64"],
];
const legalRoots = ["LICENSE", "THIRD_PARTY_LICENSES.html", "THIRD_PARTY_NOTICES.md"];
const rootManifestFiles = [
  "bin/typeul.js", "LICENSE", "README.md", "THIRD_PARTY_NOTICES.md",
  "THIRD_PARTY_LICENSES.html", "assets/licenses",
];
const lifecycleScripts = new Set([
  "preinstall", "install", "postinstall", "prepare", "prepack", "postpack",
  "prepublish", "prepublishOnly",
]);

export function commandNeedsShell(executable, platform = process.platform) {
  return platform === "win32"
    && ["npm.cmd", "npx.cmd"].includes(path.win32.basename(executable).toLowerCase());
}

function regularFile(value, label) {
  const resolved = path.resolve(value);
  const metadata = fs.lstatSync(resolved);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a regular file: ${resolved}`);
  }
  return fs.realpathSync(resolved);
}

function readJson(file, label) {
  try {
    return JSON.parse(fs.readFileSync(regularFile(file, label), "utf8"));
  } catch (error) {
    throw new Error(`invalid ${label}: ${error.message}`);
  }
}

function run(executable, args, options = {}) {
  const result = spawnSync(executable, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
    input: "",
    maxBuffer: 16 * 1024 * 1024,
    timeout: options.timeout ?? 10 * 60 * 1000,
    shell: commandNeedsShell(executable),
    windowsHide: false,
  });
  const command = [executable, ...args].join(" ");
  if (result.error) throw new Error(`${command}: ${result.error.message}`);
  if (result.signal) throw new Error(`${command}: terminated by ${result.signal}`);
  if (result.status !== 0) {
    throw new Error(`${command}: exited ${result.status}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout;
}

function exactArray(actual, expected, label) {
  if (!Array.isArray(actual) || actual.length !== expected.length
      || actual.some((value) => typeof value !== "string")
      || [...expected].sort().some((value, index) => [...actual].sort()[index] !== value)) {
    throw new Error(`${label} must be exactly: ${[...expected].sort().join(", ")}`);
  }
}

function exactObject(actual, expected, label) {
  if (!expected) {
    if (actual !== undefined) throw new Error(`packed manifest ${label} must be absent`);
    return;
  }
  const value = actual ?? {};
  const actualKeys = Object.keys(value).sort();
  const expectedKeys = Object.keys(expected).sort();
  if (!value || typeof value !== "object" || Array.isArray(value)
      || actualKeys.length !== expectedKeys.length
      || expectedKeys.some((name, index) => actualKeys[index] !== name)) {
    throw new Error(`packed manifest ${label} does not match`);
  }
  for (const [name, expectedValue] of Object.entries(expected)) {
    if (value[name] !== expectedValue) throw new Error(`packed manifest ${label}.${name} does not match`);
  }
}

export function validatePackRecord(record, expected) {
  if (!record || typeof record !== "object" || Array.isArray(record)) {
    throw new Error("npm pack record must be an object");
  }
  if (record.name !== expected.name) throw new Error(`npm pack name must be ${expected.name}`);
  if (record.version !== expected.version) throw new Error(`npm pack version must be ${expected.version}`);
  const filename = `${expected.name}-${expected.version}.tgz`;
  if (record.filename !== filename) throw new Error(`npm pack filename must be ${filename}`);
  if (!Array.isArray(record.bundled) || record.bundled.length !== 0) {
    throw new Error("npm pack bundled dependencies must be empty");
  }
  if (!Array.isArray(record.files) || record.entryCount !== expected.files.size
      || record.files.length !== expected.files.size) {
    throw new Error("npm pack files do not match the allowlist");
  }
  const seen = new Set();
  for (const file of record.files) {
    if (!file || typeof file.path !== "string" || seen.has(file.path)
        || path.posix.normalize(file.path) !== file.path || path.posix.isAbsolute(file.path)
        || file.path.startsWith("../") || !Number.isInteger(file.size) || file.size < 0) {
      throw new Error("npm pack files contain an invalid path or size");
    }
    seen.add(file.path);
    if (!expected.files.has(file.path)) throw new Error(`npm pack files contain ${file.path}`);
    if (expected.files.get(file.path) !== null && file.mode !== expected.files.get(file.path)) {
      throw new Error(`npm pack mode for ${file.path} must be ${expected.files.get(file.path)}`);
    }
  }
  exactArray([...seen], [...expected.files.keys()], "npm pack files");
  return filename;
}

export function validatePackedManifest(manifest, expected) {
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error("packed manifest must be an object");
  }
  if (manifest.name !== expected.name) throw new Error(`packed manifest name must be ${expected.name}`);
  if (manifest.version !== expected.version) throw new Error(`packed manifest version must be ${expected.version}`);
  if (Object.hasOwn(manifest, "private")) throw new Error("packed manifest must not be private");
  if (manifest.scripts !== undefined
      && (typeof manifest.scripts !== "object" || Array.isArray(manifest.scripts))) {
    throw new Error("packed manifest scripts must be an object");
  }
  for (const name of Object.keys(manifest.scripts ?? {})) {
    if (lifecycleScripts.has(name)) throw new Error(`packed manifest must not contain ${name} script`);
  }
  exactArray(manifest.files, expected.files, "packed manifest files");
  if (expected.os !== undefined) exactArray(manifest.os, [expected.os], "packed manifest os");
  if (expected.cpu !== undefined) exactArray(manifest.cpu, [expected.cpu], "packed manifest cpu");
  if (Object.hasOwn(expected, "bin")) exactObject(manifest.bin, expected.bin, "bin");
  if (Object.hasOwn(expected, "dependencies")) {
    exactObject(manifest.dependencies, expected.dependencies, "dependencies");
  }
  if (Object.hasOwn(expected, "optionalDependencies")) {
    exactObject(manifest.optionalDependencies, expected.optionalDependencies, "optionalDependencies");
  }
  if (Object.hasOwn(expected, "peerDependencies")) {
    exactObject(manifest.peerDependencies, expected.peerDependencies, "peerDependencies");
  }
  if (Object.hasOwn(expected, "peerDependenciesMeta")) {
    exactObject(manifest.peerDependenciesMeta, expected.peerDependenciesMeta, "peerDependenciesMeta");
  }
}

function licenseFiles(root, prefix) {
  const directory = path.join(root, "assets", "licenses");
  const files = [];
  function visit(current, relative) {
    for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const source = path.join(current, entry.name);
      const destination = path.posix.join(relative, entry.name);
      if (entry.isDirectory() && !entry.isSymbolicLink()) visit(source, destination);
      else if (entry.isFile() && !entry.isSymbolicLink()) {
        regularFile(source, "license");
        files.push(path.posix.join(prefix, destination));
      } else throw new Error(`license source is not regular: ${source}`);
    }
  }
  visit(directory, "");
  if (files.length === 0) throw new Error("license directory must not be empty");
  return files;
}

function pack(cwd, destination, expected) {
  const output = run(npmExecutable(), ["pack", "--json", "--pack-destination", destination], { cwd });
  let records;
  try {
    records = JSON.parse(output);
  } catch (error) {
    throw new Error(`npm pack returned invalid JSON: ${error.message}`);
  }
  if (!Array.isArray(records) || records.length !== 1) throw new Error("npm pack must return one record");
  const filename = validatePackRecord(records[0], expected);
  return regularFile(path.join(destination, filename), "packed tarball");
}

function npmExecutable() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function npxExecutable() {
  return process.platform === "win32" ? "npx.cmd" : "npx";
}

function installed(npx, args, cwd, home, env = process.env) {
  return run(npx, ["--no-install", "typeul", ...args], {
    cwd,
    env: { ...env, TYPEUL_HOME: home },
  });
}

function fakeNpm(directory) {
  fs.mkdirSync(directory, { recursive: true, mode: 0o755 });
  if (process.platform === "win32") {
    fs.writeFileSync(path.join(directory, "npm.cmd"), "@echo off\r\necho 99.0.0\r\n");
  } else {
    const file = path.join(directory, "npm");
    fs.writeFileSync(file, "#!/bin/sh\nprintf '%s\\n' '99.0.0'\n");
    fs.chmodSync(file, 0o755);
  }
}

export function prependPath(environment, directory, platform = process.platform) {
  const result = { ...environment };
  const keys = Object.keys(result).filter((name) => name.toLowerCase() === "path");
  const key = keys[0] ?? (platform === "win32" ? "Path" : "PATH");
  for (const duplicate of keys.slice(1)) delete result[duplicate];
  const delimiter = platform === "win32" ? ";" : ":";
  result[key] = `${directory}${delimiter}${result[key] ?? ""}`;
  return result;
}

function nativeDependencies(version) {
  return Object.fromEntries(supportedPairs.map(([os, cpu]) => [packageFor(os, cpu)[0], version]));
}

export function verifyTarballs(rootTgzValue, platformTgzValue, expectedVersion) {
  const rootTgz = regularFile(rootTgzValue, "root tarball");
  const platformTgz = regularFile(platformTgzValue, "platform tarball");
  if (path.dirname(rootTgz) !== path.dirname(platformTgz)) {
    throw new Error("tarballs must share one resolved temporary directory");
  }
  const [platformName, executable] = packageFor(process.platform, process.arch);
  const temporary = path.dirname(rootTgz);
  const install = path.join(temporary, "install");
  if (fs.existsSync(install)) throw new Error(`install directory must be absent: ${install}`);
  fs.mkdirSync(install, { mode: 0o755 });
  fs.writeFileSync(path.join(install, "package.json"), "{\"name\":\"typeul-package-check\",\"private\":true}\n");
  run(npmExecutable(), [
    "install", rootTgz, platformTgz, "--ignore-scripts", "--no-audit", "--no-fund",
  ], { cwd: install });

  const rootManifest = readJson(path.join(install, "node_modules", "typeul", "package.json"), "installed root manifest");
  validatePackedManifest(rootManifest, {
    name: "typeul", version: expectedVersion, files: rootManifestFiles,
    bin: { typeul: "bin/typeul.js" }, dependencies: {},
    optionalDependencies: nativeDependencies(expectedVersion), peerDependencies: {}, peerDependenciesMeta: {},
  });
  const platformManifestFiles = [executable, ...legalRoots, "licenses"];
  const platformManifest = readJson(path.join(install, "node_modules", platformName, "package.json"), "installed platform manifest");
  validatePackedManifest(platformManifest, {
    name: platformName, version: expectedVersion, files: platformManifestFiles,
    os: process.platform, cpu: process.arch, bin: null, dependencies: {},
    optionalDependencies: {}, peerDependencies: {}, peerDependenciesMeta: {},
  });

  const home = path.join(temporary, "typeul-home");
  const npx = npxExecutable();
  const version = installed(npx, ["--version"], install, home);
  if (version !== `typeul ${expectedVersion}\n`) throw new Error(`installed --version returned ${JSON.stringify(version)}`);
  const paths = installed(npx, ["paths"], install, home);
  for (const relative of ["config.toml", "sessions", "content", "themes", "cache/update.json"]) {
    if (!paths.includes(path.join(home, relative))) throw new Error(`installed paths omitted ${relative}`);
  }
  const licenses = installed(npx, ["licenses"], install, home);
  for (const text of ["===== LICENSE =====", "THIRD_PARTY_NOTICES.md", "CC0 1.0 Universal", "Sven Greb"]) {
    if (!licenses.includes(text)) throw new Error(`installed licenses omitted ${text}`);
  }
  const smoke = installed(npx, ["--smoke"], install, home);
  if (!/^smoke ok: \d+ content items, 0 sessions\n$/.test(smoke)) {
    throw new Error(`installed --smoke returned ${JSON.stringify(smoke)}`);
  }

  const fake = path.join(temporary, "fake-npm");
  fakeNpm(fake);
  const update = installed(npx, ["update"], install, home, prependPath(process.env, fake));
  for (const text of ["latest: 99.0.0", "update: npm install -g typeul@latest · npx typeul@latest"]) {
    if (!update.includes(text)) throw new Error(`installed update omitted ${text}`);
  }
}

function main() {
  const root = fs.realpathSync(process.cwd());
  const expectedVersion = validateVersions(readVersions(root));
  const [platformName, executable] = packageFor(process.platform, process.arch);
  const platformDir = path.join(root, "npm", platformName);
  const platformManifest = readJson(path.join(platformDir, "package.json"), "platform manifest");
  const rootManifest = readJson(path.join(root, "package.json"), "root manifest");
  validatePackedManifest(rootManifest, {
    name: "typeul", version: expectedVersion, files: rootManifestFiles,
    bin: { typeul: "bin/typeul.js" }, dependencies: {},
    optionalDependencies: nativeDependencies(expectedVersion), peerDependencies: {}, peerDependenciesMeta: {},
  });
  validatePackedManifest(platformManifest, {
    name: platformName,
    version: expectedVersion,
    files: [executable, ...legalRoots, "licenses"],
    os: process.platform,
    cpu: process.arch,
    bin: null,
    dependencies: {},
    optionalDependencies: {},
    peerDependencies: {},
    peerDependenciesMeta: {},
  });

  const temporary = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "typeul-package-")));
  try {
    run("cargo", ["build", "--release", "--locked"], { cwd: root });
    const binary = regularFile(path.join(root, "target", "release", executable), "release binary");
    const directVersion = run(binary, ["--version"], { cwd: root });
    if (directVersion !== `typeul ${expectedVersion}\n`) throw new Error(`release --version returned ${JSON.stringify(directVersion)}`);
    const directSmoke = run(binary, ["--smoke"], {
      cwd: root,
      env: { ...process.env, TYPEUL_HOME: path.join(temporary, "direct-home") },
    });
    if (!/^smoke ok: \d+ content items, 0 sessions\n$/.test(directSmoke)) {
      throw new Error(`release --smoke returned ${JSON.stringify(directSmoke)}`);
    }

    const staged = stagePlatform(platformDir, binary, path.join(temporary, "staged", platformName));
    const platformFiles = new Map([
      ["package.json", 0o644], [executable, process.platform === "win32" ? null : 0o755],
      ...legalRoots.map((name) => [name, 0o644]),
      ...licenseFiles(root, "licenses").map((name) => [name, 0o644]),
    ]);
    const rootFiles = new Map([
      ["package.json", 0o644], ["bin/typeul.js", 0o755], ["README.md", 0o644],
      ...legalRoots.map((name) => [name, 0o644]),
      ...licenseFiles(root, "assets/licenses").map((name) => [name, 0o644]),
    ]);
    const platformTgz = pack(staged, temporary, { name: platformName, version: expectedVersion, files: platformFiles });
    const rootTgz = pack(root, temporary, { name: "typeul", version: expectedVersion, files: rootFiles });
    verifyTarballs(rootTgz, platformTgz, expectedVersion);
    console.log(`verified root tarball: ${rootTgz}`);
    console.log(`verified platform tarball: ${platformTgz}`);
    fs.rmSync(temporary, { recursive: true });
  } catch (error) {
    console.error(`package verification failed; retained temporary directory: ${temporary}`);
    throw error;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(`verify-package: ${error.message}`);
    process.exitCode = 1;
  }
}
