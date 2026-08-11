#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
current=$(node scripts/check-versions.mjs)

if [[ -n "$(git status --porcelain)" ]]; then
  echo "working tree is dirty; commit or stash changes first" >&2
  exit 1
fi
if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "release from main" >&2
  exit 1
fi

git fetch origin main --tags
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]]; then
  echo "local main is not synced with origin/main" >&2
  exit 1
fi

echo "Current version: $current"
current_tag="$(git describe --tags --abbrev=0 2>/dev/null || printf '(none)')"
echo "Current tag: $current_tag"
version="${VERSION:-${1:-}}"
if [[ -z "$version" ]]; then
  read -r -p "Next version [$current]: " version
  version="${version:-$current}"
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version must look like 1.0.1" >&2
  exit 2
fi

node -e '
const [current, next] = process.argv.slice(1).map((value) => value.split(".").map(Number));
const ok = next[0] > current[0]
  || (next[0] === current[0] && (next[1] > current[1]
  || (next[1] === current[1] && next[2] >= current[2])));
process.exit(ok ? 0 : 1);
' "$current" "$version" || {
  echo "version must not be lower than $current" >&2
  exit 2
}

if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  echo "tag v$version already exists locally" >&2
  exit 1
fi
if git ls-remote --exit-code --tags origin "v$version" >/dev/null 2>&1; then
  echo "tag v$version already exists on origin" >&2
  exit 1
fi

echo "Releasing v$version"

base_commit="$(git rev-parse HEAD)"
release_commit=""
tag_created=false
pushed=false
version_files=(Cargo.toml Cargo.lock package.json npm/*/package.json)
restore_release() {
  status=$?
  trap - EXIT
  if [[ "$pushed" == false ]]; then
    if [[ "$tag_created" == true ]]; then
      git tag -d "v$version" >/dev/null 2>&1 || true
    fi
    if [[ -n "$release_commit" && "$(git rev-parse HEAD 2>/dev/null)" == "$release_commit" ]]; then
      git update-ref refs/heads/main "$base_commit" "$release_commit" || {
        echo "could not restore main to $base_commit" >&2
        exit "$status"
      }
    fi
    git restore --staged --worktree -- "${version_files[@]}" THIRD_PARTY_LICENSES.html
  fi
  exit "$status"
}
trap restore_release EXIT

if [[ "$version" != "$current" ]]; then
  VERSION="$version" perl -0pi -e 's/^version = ".*"/version = "$ENV{VERSION}"/m' Cargo.toml
  VERSION="$version" node <<'NODE'
const fs = require("node:fs");
const path = require("node:path");
const version = process.env.VERSION;
const write = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
const root = JSON.parse(fs.readFileSync("package.json", "utf8"));
root.version = version;
for (const name of Object.keys(root.optionalDependencies)) root.optionalDependencies[name] = version;
write("package.json", root);
for (const entry of fs.readdirSync("npm", { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const file = path.join("npm", entry.name, "package.json");
  const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  manifest.version = version;
  write(file, manifest);
}
NODE
  cargo check --offline
  node scripts/check-versions.mjs "v$version"
fi

git diff --check
make test licenses
node scripts/verify-package.mjs --check-license-tree
cargo publish --dry-run --allow-dirty --locked
npm run package-check
make pty-smoke
go run github.com/rhysd/actionlint/cmd/actionlint@914e7df21a07ef503a81201c76d2b11c789d3fca \
  .github/workflows/ci.yml .github/workflows/release.yml

if [[ "$version" != "$current" ]]; then
  git add -- "${version_files[@]}"
  git commit -m "release: v$version"
  release_commit="$(git rev-parse HEAD)"
fi

git tag "v$version"
tag_created=true
git push --atomic origin main "v$version"
pushed=true
trap - EXIT
