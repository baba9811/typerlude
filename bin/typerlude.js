#!/usr/bin/env node

const { spawnSync } = require("node:child_process");

const nativePackages = [
  { platform: "darwin", arch: "arm64", directory: "typerlude-darwin-arm64", name: "typerlude-darwin-arm64", executable: "typerlude" },
  { platform: "darwin", arch: "x64", directory: "typerlude-darwin-x64", name: "typerlude-darwin-x64", executable: "typerlude" },
  { platform: "linux", arch: "arm64", directory: "typerlude-linux-arm64", name: "typerlude-linux-arm64", executable: "typerlude" },
  { platform: "linux", arch: "x64", directory: "typerlude-linux-x64", name: "typerlude-linux-x64", executable: "typerlude" },
  { platform: "win32", arch: "arm64", directory: "typerlude-win32-arm64-msvc", name: "typerlude-win32-arm64-msvc", executable: "typerlude.exe" },
  { platform: "win32", arch: "x64", directory: "typerlude-win32-x64-msvc", name: "typerlude-win32-x64-msvc", executable: "typerlude.exe" },
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
    env: { ...process.env, TYPERLUDE_INSTALL_METHOD: "npm" },
    stdio: "inherit",
    windowsHide: false,
  });
  if (result.error) throw result.error;
  if (result.signal) process.kill(process.pid, result.signal);
  process.exitCode = result.status ?? 1;
}

if (require.main === module) run(process.argv.slice(2), process.platform, process.arch);

module.exports = { nativePackages, packageFor };
