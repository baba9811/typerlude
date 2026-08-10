import test from "node:test";
import assert from "node:assert/strict";
import { validateVersions } from "./check-versions.mjs";

test("reports every mismatched package path", () => {
  assert.throws(() => validateVersions([
    ["Cargo.toml", "1.2.3"], ["package.json", "1.2.4"],
    ["npm/typeul-linux-x64/package.json", "1.2.2"],
  ], "v1.2.3"), /package.json.*typeul-linux-x64/s);
});
