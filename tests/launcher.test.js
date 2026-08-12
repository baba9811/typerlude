const test = require("node:test");
const assert = require("node:assert/strict");
const { nativePackages, packageFor } = require("../bin/typerlude.js");

test("native package records contain only the runtime mapping", () => {
  for (const item of nativePackages) {
    assert.deepEqual(Object.keys(item).sort(), ["arch", "executable", "name", "platform"]);
  }
});

test("maps every supported native pair", () => {
  assert.deepEqual(packageFor("darwin", "arm64"), ["typerlude-darwin-arm64", "typerlude"]);
  assert.deepEqual(packageFor("darwin", "x64"), ["typerlude-darwin-x64", "typerlude"]);
  assert.deepEqual(packageFor("linux", "arm64"), ["typerlude-linux-arm64", "typerlude"]);
  assert.deepEqual(packageFor("linux", "x64"), ["typerlude-linux-x64", "typerlude"]);
  assert.deepEqual(packageFor("win32", "arm64"), [
    "typerlude-win32-arm64-msvc", "typerlude.exe",
  ]);
  assert.deepEqual(packageFor("win32", "x64"), [
    "typerlude-win32-x64-msvc", "typerlude.exe",
  ]);
});

test("rejects unshipped pairs clearly", () => {
  assert.throws(() => packageFor("freebsd", "x64"), /unsupported platform: freebsd-x64/);
});
