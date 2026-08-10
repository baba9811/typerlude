#!/usr/bin/env node

const { spawnSync } = require("node:child_process");

const packages = {
  "darwin-arm64": ["typeul-darwin-arm64", "typeul"],
  "darwin-x64": ["typeul-darwin-x64", "typeul"],
  "linux-arm64": ["typeul-linux-arm64", "typeul"],
  "linux-x64": ["typeul-linux-x64", "typeul"],
  "win32-arm64": ["typeul-win32-arm64-msvc", "typeul.exe"],
  "win32-x64": ["typeul-win32-x64-msvc", "typeul.exe"],
};

function packageFor(platform, arch) {
  const result = packages[`${platform}-${arch}`];
  if (!result) throw new Error(`unsupported platform: ${platform}-${arch}`);
  return result;
}

function run(argv, platform, arch) {
  const [packageName, executable] = packageFor(platform, arch);
  let binary;
  try {
    binary = require.resolve(`${packageName}/${executable}`);
  } catch {
    throw new Error(
      `optional package ${packageName} is unavailable; reinstall without --no-optional`,
    );
  }

  const result = spawnSync(binary, argv, {
    cwd: process.cwd(),
    env: { ...process.env, TYPEUL_INSTALL_METHOD: "npm" },
    stdio: "inherit",
    windowsHide: false,
  });
  if (result.error) throw result.error;
  if (result.signal) process.kill(process.pid, result.signal);
  process.exitCode = result.status ?? 1;
}

if (require.main === module) run(process.argv.slice(2), process.platform, process.arch);

module.exports = { packageFor };
