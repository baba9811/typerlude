import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const { nativePackages } = createRequire(import.meta.url)("../bin/typerlude.js");
const supported = new Map(nativePackages.map((item) => [item.directory, item]));
const legalRoots = ["LICENSE", "THIRD_PARTY_LICENSES.html", "THIRD_PARTY_NOTICES.md"];

function realDirectory(value, label) {
  const resolved = path.resolve(value);
  const metadata = fs.lstatSync(resolved);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a real directory: ${resolved}`);
  }
  return fs.realpathSync(resolved);
}

function regularFile(value, label) {
  const resolved = path.resolve(value);
  const metadata = fs.lstatSync(resolved);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a regular file: ${resolved}`);
  }
  return fs.realpathSync(resolved);
}

function inside(root, candidate, label) {
  const relative = path.relative(root, candidate);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`${label} is outside the package root: ${candidate}`);
  }
}

function sameStrings(actual, expected) {
  if (!Array.isArray(actual) || actual.length !== expected.length
      || actual.some((value) => typeof value !== "string")) return false;
  const sorted = [...actual].sort();
  return [...expected].sort().every((value, index) => sorted[index] === value);
}

function outputDirectory(value) {
  const output = path.resolve(value);
  let metadata;
  try {
    metadata = fs.lstatSync(output);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  if (metadata) {
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      throw new Error(`output must be a real directory: ${output}`);
    }
    if (fs.readdirSync(output).length !== 0) throw new Error(`output must be empty: ${output}`);
    return fs.realpathSync(output);
  }

  let ancestor = path.dirname(output);
  while (true) {
    try {
      fs.lstatSync(ancestor);
      break;
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    const parent = path.dirname(ancestor);
    if (parent === ancestor) throw new Error(`output has no existing parent: ${output}`);
    ancestor = parent;
  }
  const relative = path.relative(ancestor, output);
  return path.join(realDirectory(ancestor, "output parent"), relative);
}

function licenseFiles(root) {
  const directory = realDirectory(path.join(root, "assets", "licenses"), "license directory");
  inside(root, directory, "license directory");
  const files = [];
  function visit(current, relative) {
    for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const source = path.join(current, entry.name);
      const destination = path.join(relative, entry.name);
      if (entry.isDirectory() && !entry.isSymbolicLink()) {
        visit(source, destination);
      } else if (entry.isFile() && !entry.isSymbolicLink()) {
        const file = regularFile(source, "license");
        inside(root, file, "license");
        files.push([file, destination]);
      } else {
        throw new Error(`license must be a regular file or real directory: ${source}`);
      }
    }
  }
  visit(directory, "");
  if (files.length === 0) throw new Error("license directory must not be empty");
  return files;
}

function copy(source, destination, mode) {
  fs.mkdirSync(path.dirname(destination), { recursive: true, mode: 0o755 });
  fs.copyFileSync(source, destination, fs.constants.COPYFILE_EXCL);
  if (process.platform !== "win32") fs.chmodSync(destination, mode);
}

export function stagePlatform(packageDirValue, binaryValue, outputValue) {
  const packageDir = realDirectory(packageDirValue, "package directory");
  if (path.basename(path.dirname(packageDir)) !== "npm") {
    throw new Error(`package directory must be under npm: ${packageDir}`);
  }
  const root = realDirectory(path.dirname(path.dirname(packageDir)), "package root");
  inside(root, packageDir, "package directory");

  const manifestFile = regularFile(path.join(packageDir, "package.json"), "manifest");
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestFile, "utf8"));
  } catch (error) {
    throw new Error(`invalid native package manifest: ${error.message}`);
  }
  const expected = supported.get(path.basename(packageDir));
  if (!expected || manifest.name !== expected.name) {
    throw new Error(`manifest name does not match a supported package directory: ${manifest.name}`);
  }
  if (!Array.isArray(manifest.os) || manifest.os.length !== 1 || manifest.os[0] !== expected.platform) {
    throw new Error(`manifest os must be ${expected.platform}`);
  }
  if (!Array.isArray(manifest.cpu) || manifest.cpu.length !== 1 || manifest.cpu[0] !== expected.arch) {
    throw new Error(`manifest cpu must be ${expected.arch}`);
  }
  const expectedFiles = [expected.executable, ...legalRoots, "licenses"].sort();
  if (!sameStrings(manifest.files, expectedFiles)) {
    throw new Error(`manifest files must be exactly: ${expectedFiles.join(", ")}`);
  }

  const binary = regularFile(binaryValue, "binary");
  inside(root, binary, "binary");
  const roots = legalRoots.map((name) => [regularFile(path.join(root, name), name), name]);
  const licenses = licenseFiles(root);
  const output = outputDirectory(outputValue);

  fs.mkdirSync(output, { recursive: true, mode: 0o755 });
  copy(manifestFile, path.join(output, "package.json"), 0o644);
  for (const [source, name] of roots) copy(source, path.join(output, name), 0o644);
  for (const [source, name] of licenses) copy(source, path.join(output, "licenses", name), 0o644);
  copy(binary, path.join(output, expected.executable), 0o755);
  return output;
}

function argumentsFrom(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const option = argv[index];
    if (!['--package-dir', '--binary', '--out'].includes(option)
        || Object.hasOwn(values, option) || argv[index + 1] === undefined) {
      throw new Error("usage: stage-platform-package.mjs --package-dir DIR --binary FILE --out DIR");
    }
    values[option] = argv[index + 1];
  }
  if (Object.keys(values).length !== 3) {
    throw new Error("usage: stage-platform-package.mjs --package-dir DIR --binary FILE --out DIR");
  }
  return values;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const values = argumentsFrom(process.argv.slice(2));
    stagePlatform(values["--package-dir"], values["--binary"], values["--out"]);
  } catch (error) {
    console.error(`stage-platform-package: ${error.message}`);
    process.exitCode = 1;
  }
}
