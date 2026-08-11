const test = require("node:test");
const assert = require("node:assert/strict");
const { packageFor } = require("../bin/typeul.js");

test("maps every supported native pair", () => {
  assert.deepEqual(packageFor("darwin", "arm64"), ["typeul-darwin-arm64", "typeul"]);
  assert.deepEqual(packageFor("darwin", "x64"), ["typeul-darwin-x64", "typeul"]);
  assert.deepEqual(packageFor("linux", "arm64"), ["typeul-linux-arm64", "typeul"]);
  assert.deepEqual(packageFor("linux", "x64"), ["typeul-linux-x64", "typeul"]);
  assert.deepEqual(packageFor("win32", "arm64"), [
    "@baba9811/typeul-win32-arm64-msvc", "typeul.exe", "typeul-win32-arm64-msvc",
  ]);
  assert.deepEqual(packageFor("win32", "x64"), [
    "@baba9811/typeul-win32-x64-msvc", "typeul.exe", "typeul-win32-x64-msvc",
  ]);
});

test("rejects unshipped pairs clearly", () => {
  assert.throws(() => packageFor("freebsd", "x64"), /unsupported platform: freebsd-x64/);
});
