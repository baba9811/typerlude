# Contributing to Typerlude

Bug reports, feature ideas, and focused pull requests are welcome.

## Before opening an issue

- Search existing issues and use the matching form.
- Report vulnerabilities privately through [GitHub Security Advisories](https://github.com/baba9811/typerlude/security/advisories/new).
- Never attach credentials, `.env` files, private practice text, or local config/session files.

A useful bug report includes the Typerlude version, install method, OS, terminal, reproduction
steps, expected result, and actual result. A useful feature request starts with the user problem,
not an implementation wish list.

## Pull requests

1. Fork the repository and create a focused branch.
2. Make the smallest change that solves the issue and add a regression test for behavior changes.
3. Run `make test`.
4. Update user-facing documentation when behavior changes.
5. Open a pull request with the problem, solution, and verification performed.

Typerlude requires Rust 1.88 or newer and supports Node.js 18 or newer for its launcher and
tooling. The repository's development toolchain is declared in `rust-toolchain.toml`.

Content contributions must include a redistributable license and verifiable provenance. Follow the
[content-pack guide](docs/content-packs.md); do not commit raw exports, temporary archives, or text
you do not have the right to redistribute. Dependency or bundled-license changes must keep
`LICENSE`, `THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES.html` accurate.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
