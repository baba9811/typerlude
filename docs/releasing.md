# Typerlude releases / Typerlude 배포

Typerlude publishes from `baba9811/typerlude` through GitHub Actions trusted publishing.
`make release` is the only release entry point. Registry publication is OIDC-only, and the
workflow never accepts long-lived registry credentials.

## Trusted-publisher bindings / 신뢰 게시자 연결

Both registry jobs use the exact GitHub environment `release`, which is restricted to tags
matching `v*.*.*`. The jobs run on GitHub-hosted runners with `id-token: write`.

The crates.io binding is:

- crate: `typerlude`
- repository: `baba9811/typerlude`
- workflow filename: `release.yml`
- environment: `release`

Each npm package has the same repository, workflow filename, environment, and permission to
publish:

- `typerlude`
- `typerlude-darwin-arm64`
- `typerlude-darwin-x64`
- `typerlude-linux-arm64`
- `typerlude-linux-x64`
- `typerlude-win32-arm64-msvc`
- `typerlude-win32-x64-msvc`

Trusted publication depends on those values matching exactly. npm packages are published in the
listed native-package order with `typerlude` last; Cargo is published before npm.

## Release / 배포

Start from a clean, synchronized `main` and run:

```bash
git switch main
git pull --ff-only origin main
make release
```

The prompt shows the current version and tag, then asks for the next version. The script updates
Cargo and all seven npm manifests, runs the complete local gates, creates a signed annotated tag,
and atomically pushes `main` with that tag. The tag starts the protected release workflow.

The workflow validates the tag and its ancestry, builds six native targets, verifies the exact
artifact closure, creates a draft GitHub release, publishes Cargo through crates.io OIDC, publishes
the six native npm packages and root package through npm OIDC, then makes the GitHub release public.
Do not publish individual registry packages or the draft release manually.

GitHub immutable releases protect only releases created after the setting is enabled. A release
that predates the setting does not become immutable retroactively.

## Verify / 확인

After the workflow succeeds, verify the released version and all seven npm provenance records:

```bash
tag="$(git describe --tags --abbrev=0)"
version="${tag#v}"
cargo info "typerlude@$version"
for package in \
  typerlude typerlude-darwin-arm64 typerlude-darwin-x64 typerlude-linux-arm64 \
  typerlude-linux-x64 typerlude-win32-arm64-msvc typerlude-win32-x64-msvc; do
  npm view "$package@$version" name version repository dist.integrity dist.attestations --json
done
gh release view "$tag" --repo baba9811/typerlude --json isDraft,isImmutable,assets,url
temporary="$(mktemp -d)"
gh release download "$tag" --repo baba9811/typerlude --dir "$temporary"
(cd "$temporary" && sha256sum --check SHA256SUMS)
rm -rf -- "$temporary"
```

The GitHub release must contain 13 payload files plus `SHA256SUMS`. Inspect at least one tar.gz,
one zip, the Cargo crate, the root npm tarball, and one native npm tarball when changing packaging.

## Partial publication recovery / 부분 배포 복구

The registries are not transactionally coupled. If a registry outage leaves a draft or partial npm
publication, inspect the registry state and, while its artifacts remain available for one day, rerun
only the failed jobs from the same workflow run:

```bash
run_id=REPLACE_WITH_FAILED_RUN_ID
gh run view "$run_id" --repo baba9811/typerlude
gh run rerun "$run_id" --failed --repo baba9811/typerlude
gh run watch "$run_id" --repo baba9811/typerlude --exit-status
```

After artifact expiry, dispatch the same tag as both workflow ref and input; never select a branch:

```bash
tag=v1.0.0
gh workflow run release.yml --repo baba9811/typerlude --ref "$tag" -f tag="$tag"
```

The workflow skips an existing Cargo version only when its checksum is identical. For npm, it skips
an existing version only when the local tarball SHA-512 SRI matches `dist.integrity`; every other
query or integrity error stops publication. Do not bump the version or publish tarballs manually
to repair a partial run.

Primary references: [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/),
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing),
[GitHub environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments),
and [GitHub OIDC](https://docs.github.com/en/actions/reference/security/oidc).
