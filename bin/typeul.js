#!/usr/bin/env node

const { spawnSync } = require("node:child_process");

const nativePackages = [
  { platform: "darwin", arch: "arm64", directory: "typeul-darwin-arm64", name: "typeul-darwin-arm64", executable: "typeul" },
  { platform: "darwin", arch: "x64", directory: "typeul-darwin-x64", name: "typeul-darwin-x64", executable: "typeul" },
  { platform: "linux", arch: "arm64", directory: "typeul-linux-arm64", name: "typeul-linux-arm64", executable: "typeul" },
  { platform: "linux", arch: "x64", directory: "typeul-linux-x64", name: "typeul-linux-x64", executable: "typeul" },
  { platform: "win32", arch: "arm64", directory: "typeul-win32-arm64-msvc", name: "@baba9811/typeul-win32-arm64-msvc", executable: "typeul.exe" },
  { platform: "win32", arch: "x64", directory: "typeul-win32-x64-msvc", name: "@baba9811/typeul-win32-x64-msvc", executable: "typeul.exe" },
];

function packageFor(platform, arch) {
  const result = nativePackages.find((item) => item.platform === platform && item.arch === arch);
  if (!result) throw new Error(`unsupported platform: ${platform}-${arch}`);
  return result.name === result.directory
    ? [result.name, result.executable]
    : [result.name, result.executable, result.directory];
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

module.exports = { nativePackages, packageFor };
