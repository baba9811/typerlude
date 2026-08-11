import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const packages = [
  "typeul-darwin-arm64",
  "typeul-darwin-x64",
  "typeul-linux-arm64",
  "typeul-linux-x64",
  "typeul-win32-arm64-msvc",
  "typeul-win32-x64-msvc",
  "typeul",
];
const registry = "https://registry.npmjs.org/";

function defaultRunNpm(args, capture) {
  const executable = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = spawnSync(executable, args, capture
    ? { encoding: "utf8", maxBuffer: 64 * 1024, stdio: ["ignore", "pipe", "pipe"] }
    : { stdio: ["ignore", "inherit", "inherit"] });
  return {
    status: result.status,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    error: result.error,
  };
}

function parseJson(value) {
  try {
    return JSON.parse(value.trim());
  } catch {
    return null;
  }
}

function isE404(result) {
  return [result.stdout, result.stderr].some((output) => {
    const parsed = parseJson(output);
    return parsed?.error?.code === "E404" || parsed?.code === "E404";
  });
}

function isSha512Integrity(value) {
  if (typeof value !== "string" || !/^sha512-[A-Za-z0-9+/]+={0,2}$/.test(value)) return false;
  const encoded = value.slice("sha512-".length);
  const bytes = Buffer.from(encoded, "base64");
  return bytes.length === 64 && bytes.toString("base64") === encoded;
}

function localIntegrity(tarball) {
  return `sha512-${crypto.createHash("sha512").update(fs.readFileSync(tarball)).digest("base64")}`;
}

function remoteIntegrity(packageName, version, runNpm) {
  const spec = `${packageName}@${version}`;
  const result = runNpm([
    "view",
    spec,
    "dist.integrity",
    "--json",
    "--loglevel=silent",
    `--registry=${registry}`,
  ], true);
  if (result.error) throw new Error(`npm registry query failed for ${spec}`);
  if (result.status === 0) {
    const integrity = parseJson(result.stdout);
    if (!isSha512Integrity(integrity)) {
      throw new Error(`npm registry returned malformed integrity for ${spec}`);
    }
    return integrity;
  }
  if (isE404(result)) return null;
  throw new Error(`npm registry query failed for ${spec}`);
}

export function publishNpmPackages({
  version,
  distDir = "dist",
  provenance = false,
  runNpm = defaultRunNpm,
}) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`invalid release version: ${version}`);

  for (const packageName of packages) {
    const spec = `${packageName}@${version}`;
    const tarball = path.resolve(distDir, `${packageName}-${version}.tgz`);
    if (!fs.lstatSync(tarball).isFile()) throw new Error(`missing npm tarball for ${spec}`);
    const local = localIntegrity(tarball);
    const remote = remoteIntegrity(packageName, version, runNpm);
    if (remote !== null) {
      if (remote !== local) throw new Error(`integrity mismatch for ${spec}`);
      console.log(`Skipping ${spec}: registry integrity matches`);
      continue;
    }

    console.log(`Publishing ${spec}`);
    const args = ["publish", "--ignore-scripts", "--access", "public"];
    if (provenance) args.push("--provenance");
    args.push(`--registry=${registry}`, tarball);
    const result = runNpm(args, false);
    if (result.error || result.status !== 0) throw new Error(`npm publish failed for ${spec}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [version, ...options] = process.argv.slice(2);
  if (options.some((option) => option !== "--provenance") || options.length > 1) {
    console.error("Usage: node scripts/publish-npm-packages.mjs VERSION [--provenance]");
    process.exitCode = 1;
  } else {
    try {
      publishNpmPackages({ version, provenance: options[0] === "--provenance" });
    } catch (error) {
      console.error(error.message);
      process.exitCode = 1;
    }
  }
}
