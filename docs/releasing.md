# Typeul release handoff / 배포 인수인계

이 문서는 공개 저장소 `baba9811/typeul`의 소스 공개와 일반 CI가 완료된 뒤, 최초
레지스트리 bootstrap과 이후 OIDC 전용 배포에 실제로 남는 maintainer
checklist입니다. 명령의 토큰 자리에는 실제 값을 넣거나 로그로 출력하지 마세요.

The workflow is deliberately not cross-registry atomic. A registry outage can leave Cargo or
some npm packages published while the GitHub release remains a draft. Inspect registry state
before retrying; never assume a failed run published nothing.

## Partial npm publication recovery / npm 부분 배포 복구

Do not bump the version or publish individual tarballs manually after a partial npm success.
Rerun the failed protected workflow jobs while the immutable release artifact is retained:

```bash
run_id=REPLACE_WITH_FAILED_RUN_ID
gh run view "$run_id" --repo baba9811/typeul
gh run rerun "$run_id" --failed --repo baba9811/typeul
gh run watch "$run_id" --repo baba9811/typeul --exit-status
```

For each of the seven packages, in native-package order with `typeul` last, the workflow queries
only `https://registry.npmjs.org/`. A structured npm `E404` means the version is absent and the
exact local tarball is published. If the version exists, the workflow computes that tarball's
SHA-512 SRI and skips it only when it exactly equals `dist.integrity`. An integrity mismatch,
malformed response, authentication or network error, or any non-`E404` query failure stops the
job before later packages. Bootstrap reruns retain explicit provenance; OIDC reruns retain token
isolation and trusted-publication provenance. Never put an auth token in these commands or logs.

## 1. GitHub release protections / GitHub 배포 보호 설정

- In GitHub `Settings → Rules → Rulesets`, protect `main`: require the reviewed CI checks,
  block force-pushes and deletion, and restrict direct updates except the release maintainer's
  atomic version-commit-and-tag push from `make release`.
- Add an active tag ruleset for `v*.*.*`: restrict creation to release maintainers and block
  updates and deletion. The workflow requires a semver tag that points to the checked-out commit
  and is reachable from `origin/main`; mismatched and non-semver tags fail before builds begin.
- Create the `release` environment for registry credentials and OIDC. Under **Deployment branches
  and tags**, select only tags matching `v*.*.*` and disallow administrator bypass. This prevents
  any other ref from receiving the bootstrap secrets. No reviewer is required, so a valid tag
  starts publication automatically. Registry jobs must use this exact environment name.
- Keep Actions workflow permissions read-only by default. The workflow grants only its draft
  and final release jobs `contents: write`, and its two registry jobs `id-token: write`.

Relevant settings: [rulesets](https://github.com/baba9811/typeul/settings/rules),
[environments](https://github.com/baba9811/typeul/settings/environments), and
[Actions](https://github.com/baba9811/typeul/settings/actions).

## 2. Recheck all eight names / 이름 8개 재확인

Name ownership is first-come-first-served. Check immediately before bootstrap:

- crates.io: `typeul`
- npm: `typeul`
- npm: `typeul-darwin-arm64`
- npm: `typeul-darwin-x64`
- npm: `typeul-linux-arm64`
- npm: `typeul-linux-x64`
- npm: `@baba9811/typeul-win32-arm64-msvc`
- npm: `@baba9811/typeul-win32-x64-msvc`

```bash
curl -fsS https://crates.io/api/v1/crates/typeul
for package in \
  typeul typeul-darwin-arm64 typeul-darwin-x64 typeul-linux-arm64 \
  typeul-linux-x64 @baba9811/typeul-win32-arm64-msvc \
  @baba9811/typeul-win32-x64-msvc; do
  npm view "$package" name version --json
done
```

For an available name these read-only commands are expected to return a not-found response.
Any existing result is a stop condition until ownership is resolved.

## 3. One-time npm bootstrap credential / npm 최초 1회 자격 증명

The `typeul` crate already exists and every later Cargo release uses OIDC. Never add a
`CRATES_TOKEN` to this repository again. Until all seven npm packages have been created, make one
**new Typeul-only, short-lived** npm credential immediately before tagging:

1. On [npm access tokens](https://www.npmjs.com/settings/~/tokens), create a granular token with
   the shortest allowed expiry, **Packages and scopes: Read and write**, **All Packages** (some
   required names do not exist yet), no organization-management access, and **Bypass 2FA enabled**.
   This exception is only for non-interactive first creation.
2. In the Typeul `release` environment, add only environment secret `NPM_TOKEN` and environment
   variable `TYPEUL_NPM_BOOTSTRAP=1`.

The ignored, mode-`600` `.env` in the main worktree is only a local input convenience; GitHub
Actions cannot read it. After filling its value, upload it without printing it:

```bash
(
  set +x
  set -euo pipefail
  source .env
  : "${NPM_TOKEN:?fill NPM_TOKEN in .env}"
  printf '%s' "$NPM_TOKEN" | gh secret set NPM_TOKEN --env release --repo baba9811/typeul
  gh variable set TYPEUL_NPM_BOOTSTRAP --body 1 --env release --repo baba9811/typeul
  rm -- .env
)
```

Cargo never reads a registry secret. npm requires `NPM_TOKEN` only while
`TYPEUL_NPM_BOOTSTRAP=1`. Values are never printed.

GitHub does not reveal, copy, or transfer secret values between repositories. Never reuse a
Practicode credential.

## 4. First release tag / 최초 배포 태그

Only after the repository, protections, environment, crates.io trusted publisher, npm bootstrap
secret, and npm bootstrap variable exist, run the same guarded entry point used for later releases:

```bash
git switch main
git pull --ff-only origin main
VERSION=1.0.0 make release
```

For a later release, run `VERSION=1.0.1 make release` or use the interactive `make release`
prompt. The command requires a
clean synchronized `main`, updates Cargo and all seven npm package versions, runs the complete
package and PTY gates, creates the version commit when needed, creates a semver tag, then
atomically pushes `main` and that signed tag. Git must have a signing key configured;
the workflow rejects lightweight, unsigned, or indirectly nested tags. `VERSION=1.0.0` tags the already
synchronized initial version without creating an empty version commit.

Open the [Release workflow](https://github.com/baba9811/typeul/actions/workflows/release.yml),
and watch the tag run. The workflow builds and validates six native archives, creates a verified
draft, publishes Cargo, publishes six native npm packages in fixed order and the root package last,
then makes the same GitHub release public. Do not publish the draft manually.

## 5. Configure trusted publishers / OIDC 설정

crates.io cannot configure trusted publishing until the crate exists. After `typeul 1.0.0` is
visible, open `typeul → Settings → Trusted Publishing → Add` and enter exactly:

- Publisher: GitHub
- Repository owner: `baba9811`
- Repository name: `typeul`
- Workflow filename: `release.yml`
- Environment: `release`

For **each of the seven npm packages**, configure one GitHub Actions Trusted Publisher with:

- Repository: `baba9811/typeul`
- Workflow filename: `release.yml` (filename only, including extension)
- Environment: `release`
- Allowed action: `npm publish`

The seven npm settings pages are:

- <https://www.npmjs.com/package/typeul/access>
- <https://www.npmjs.com/package/typeul-darwin-arm64/access>
- <https://www.npmjs.com/package/typeul-darwin-x64/access>
- <https://www.npmjs.com/package/typeul-linux-arm64/access>
- <https://www.npmjs.com/package/typeul-linux-x64/access>
- <https://www.npmjs.com/package/@baba9811/typeul-win32-arm64-msvc/access>
- <https://www.npmjs.com/package/@baba9811/typeul-win32-x64-msvc/access>

Configure these records in the npm web UI. The `npm trust` command shown by newer npm
documentation is not available in the workflow's pinned npm 11.5.1 minimum.

Trusted publication requires the exact public repository, workflow filename, environment,
GitHub-hosted runner, Node 22.14.0+, npm 11.5.1+, and `id-token: write`. Normal OIDC publication
creates provenance automatically. The one-time token-authenticated bootstrap explicitly passes
`--provenance` so bootstrap releases receive the same public GitHub Actions attestations.

## 6. Verify, remove bootstrap, and prove OIDC / 검증·삭제·OIDC 확인

Verify all published material before removing temporary access:

```bash
cargo info typeul@1.0.1
for package in \
  typeul typeul-darwin-arm64 typeul-darwin-x64 typeul-linux-arm64 \
  typeul-linux-x64 @baba9811/typeul-win32-arm64-msvc \
  @baba9811/typeul-win32-x64-msvc; do
  npm view "$package@1.0.1" name version repository dist.integrity dist.attestations --json
done
gh release view v1.0.1 --repo baba9811/typeul --json isDraft,isImmutable,assets,url
temporary="$(mktemp -d)"
gh release download v1.0.1 --repo baba9811/typeul --dir "$temporary"
(cd "$temporary" && sha256sum --check SHA256SUMS)
```

Also inspect [crates.io/typeul](https://crates.io/crates/typeul), all seven npm package pages and
their provenance records, and the [v1.0.1 release](https://github.com/baba9811/typeul/releases/tag/v1.0.1).
Confirm the release has exactly 13 payload files plus `SHA256SUMS`, and manually inspect at least
one tar.gz, one zip, the Cargo crate, the root npm tarball, and a native npm tarball.

Then, in this order:

1. Delete environment variable `TYPEUL_NPM_BOOTSTRAP`.
2. Delete Typeul environment secret `NPM_TOKEN`.
3. Revoke the npm bootstrap token; confirm it no longer appears as active.
4. For the next synchronized semver tag, require both registry jobs to succeed through OIDC with
   no registry token secret. Re-run the registry version,
   provenance, checksum, asset-closure, and public-release checks above for that version.

Do not restore a long-lived token as an OIDC fallback. If either trusted-publisher binding is
wrong, fix the registry configuration and rerun only after checking partial publication state.

Primary references: [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/),
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing),
[GitHub environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments),
and [GitHub OIDC permissions](https://docs.github.com/en/actions/reference/security/oidc).
