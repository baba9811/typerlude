import test from "node:test";
import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const expectedPackages = [
  ["typeul-darwin-arm64", "typeul-darwin-arm64"],
  ["typeul-darwin-x64", "typeul-darwin-x64"],
  ["typeul-linux-arm64", "typeul-linux-arm64"],
  ["typeul-linux-x64", "typeul-linux-x64"],
  ["@baba9811/typeul-win32-arm64-msvc", "typeul-win32-arm64-msvc"],
  ["@baba9811/typeul-win32-x64-msvc", "typeul-win32-x64-msvc"],
  ["@baba9811/typeul", "typeul"],
];
const registry = "https://registry.npmjs.org/";

async function loadPublisher() {
  const publisher = await import("./publish-npm-packages.mjs").catch(() => null);
  assert.ok(publisher, "the resumable npm publisher must exist");
  return publisher;
}

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "typeul-publish-npm-"));
  const version = "1.2.3";
  for (const [packageName, tarballName] of expectedPackages) {
    fs.writeFileSync(path.join(root, `${tarballName}-${version}.tgz`), `tarball:${packageName}`);
  }
  return { root, version };
}

function integrity(file) {
  return `sha512-${crypto.createHash("sha512").update(fs.readFileSync(file)).digest("base64")}`;
}

function e404() {
  return {
    status: 1,
    stdout: "",
    stderr: JSON.stringify({ error: { code: "E404", summary: "Not Found", detail: "" } }),
  };
}

test("queries E404 then publishes every exact tarball in fixed root-last order", async () => {
  const { publishNpmPackages } = await loadPublisher();
  const { root, version } = fixture();
  const relativeRoot = path.relative(process.cwd(), root);
  const calls = [];
  try {
    publishNpmPackages({
      version,
      distDir: relativeRoot,
      provenance: true,
      runNpm(args, capture) {
        calls.push({ args, capture });
        return args[0] === "view" ? e404() : { status: 0, stdout: "", stderr: "" };
      },
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }

  assert.equal(calls.length, 14);
  for (const [index, [packageName, tarballName]] of expectedPackages.entries()) {
    assert.deepEqual(calls[index * 2], {
      args: [
        "view",
        `${packageName}@${version}`,
        "dist.integrity",
        "--json",
        "--loglevel=silent",
        `--registry=${registry}`,
      ],
      capture: true,
    });
    assert.deepEqual(calls[index * 2 + 1], {
      args: [
        "publish",
        "--ignore-scripts",
        "--access",
        "public",
        "--provenance",
        `--registry=${registry}`,
        path.join(root, `${tarballName}-${version}.tgz`),
      ],
      capture: false,
    });
  }
});

test("a partial rerun skips exact remote SRI and publishes only absent packages", async () => {
  const { publishNpmPackages } = await loadPublisher();
  const { root, version } = fixture();
  const absent = "typeul-linux-arm64";
  const calls = [];
  try {
    publishNpmPackages({
      version,
      distDir: root,
      runNpm(args, capture) {
        calls.push({ args, capture });
        if (args[0] === "publish") return { status: 0, stdout: "", stderr: "" };
        const packageName = args[1].slice(0, -(`@${version}`.length));
        if (packageName === absent) return e404();
        const tarballName = expectedPackages.find(([name]) => name === packageName)[1];
        return {
          status: 0,
          stdout: JSON.stringify(integrity(path.join(root, `${tarballName}-${version}.tgz`))),
          stderr: "",
        };
      },
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }

  assert.deepEqual(
    calls.filter(({ args }) => args[0] === "view").map(({ args }) => args[1]),
    expectedPackages.map(([packageName]) => `${packageName}@${version}`),
  );
  assert.deepEqual(
    calls.filter(({ args }) => args[0] === "publish").map(({ args }) => args.at(-1)),
    [path.join(root, `${absent}-${version}.tgz`)],
  );
});

test("an integrity mismatch aborts before publishing or querying later packages", async () => {
  const { publishNpmPackages } = await loadPublisher();
  const { root, version } = fixture();
  const calls = [];
  try {
    assert.throws(
      () => publishNpmPackages({
        version,
        distDir: root,
        runNpm(args, capture) {
          calls.push({ args, capture });
          const packageName = args[1].slice(0, -(`@${version}`.length));
          const [firstName, firstTarball] = expectedPackages[0];
          const remote = packageName === firstName
            ? integrity(path.join(root, `${firstTarball}-${version}.tgz`))
            : `sha512-${crypto.createHash("sha512").update("different tarball").digest("base64")}`;
          return { status: 0, stdout: JSON.stringify(remote), stderr: "" };
        },
      }),
      /integrity mismatch for typeul-darwin-x64@1\.2\.3/,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
  assert.deepEqual(calls.map(({ args }) => args[1]), [
    "typeul-darwin-arm64@1.2.3",
    "typeul-darwin-x64@1.2.3",
  ]);
});

test("malformed, auth, network, and non-E404 failures abort without leaking output", async () => {
  const { publishNpmPackages } = await loadPublisher();
  for (const response of [
    { status: 0, stdout: "not-json", stderr: "" },
    { status: 1, stdout: "", stderr: JSON.stringify({ error: { code: "E401" } }) },
    { status: 1, stdout: "", stderr: "network failed with secret-token" },
  ]) {
    const { root, version } = fixture();
    let calls = 0;
    try {
      let error;
      try {
        publishNpmPackages({
          version,
          distDir: root,
          runNpm() {
            calls += 1;
            return response;
          },
        });
      } catch (caught) {
        error = caught;
      }
      assert.ok(error);
      assert.equal(calls, 1);
      assert.doesNotMatch(error.message, /secret-token/);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
});
