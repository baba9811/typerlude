import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const { nativePackages } = createRequire(import.meta.url)("../bin/typerlude.js");
const versionPattern = /^\d+\.\d+\.\d+$/;
const rootPackageName = "typerlude";

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function cargoPackageVersion(cargoToml) {
  const packageSection = cargoToml.split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
  const version = packageSection?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) throw new Error("Cargo.toml is missing [package].version");
  return version;
}

function cargoLockVersion(cargoLock) {
  const records = cargoLock.split(/^\[\[package\]\]\s*$/m)
    .filter((packageRecord) => /^name\s*=\s*"typerlude"\s*$/m.test(packageRecord));
  if (records.length !== 1) throw new Error("Cargo.lock must contain exactly one typerlude package");
  const version = records[0].match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) throw new Error("Cargo.lock typerlude package is missing version");
  return version;
}

export function readVersions(root) {
  const cargoToml = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const cargoLock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
  const rootPackage = readJson(path.join(root, "package.json"));
  const manifestNames = fs.readdirSync(path.join(root, "npm"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  const expectedDirectories = nativePackages.map(({ directory }) => directory).sort();
  const expectedNames = nativePackages.map(({ name }) => name).sort();

  if (rootPackage.name !== rootPackageName) {
    throw new Error(`root npm package must be ${rootPackageName}`);
  }
  if (manifestNames.join("\n") !== expectedDirectories.join("\n")) {
    throw new Error(`Native manifests must be exactly: ${expectedDirectories.join(", ")}`);
  }
  if (Object.keys(rootPackage.optionalDependencies ?? {}).sort().join("\n") !== expectedNames.join("\n")) {
    throw new Error(`optionalDependencies must be exactly: ${expectedNames.join(", ")}`);
  }

  const records = [
    ["Cargo.toml", cargoPackageVersion(cargoToml)],
    ["Cargo.lock", cargoLockVersion(cargoLock)],
    ["package.json", rootPackage.version],
  ];
  for (const { directory, name } of nativePackages) {
    const manifest = readJson(path.join(root, "npm", directory, "package.json"));
    if (manifest.name !== name) throw new Error(`npm/${directory}/package.json has name ${manifest.name}`);
    records.push([`npm/${directory}/package.json`, manifest.version]);
    records.push([`package.json optionalDependencies.${name}`, rootPackage.optionalDependencies[name]]);
  }
  return records;
}

export function validateVersions(records, optionalTag) {
  const version = records[0]?.[1];
  if (!version || !versionPattern.test(version)) throw new Error(`Invalid version: ${version}`);

  const invalid = records.filter(([, candidate]) => !versionPattern.test(candidate));
  if (invalid.length) throw new Error(`Invalid versions: ${invalid.map(([file, candidate]) => `${file} (${candidate})`).join(", ")}`);

  const mismatches = records.filter(([, candidate]) => candidate !== version);
  if (mismatches.length) throw new Error(`Version mismatch: ${mismatches.map(([file, candidate]) => `${file} (${candidate})`).join(", ")}`);
  if (optionalTag !== undefined && optionalTag !== `v${version}`) {
    throw new Error(`Tag must be v${version}: ${optionalTag}`);
  }
  return version;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  console.log(validateVersions(readVersions(process.cwd()), process.argv[2]));
}
