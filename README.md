# Typeul

[English](https://github.com/baba9811/typeul/blob/main/README.md) |
[한국어](https://github.com/baba9811/typeul/blob/main/README.ko.md)

[![CI](https://github.com/baba9811/typeul/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/baba9811/typeul/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/typeul?logo=rust)](https://crates.io/crates/typeul)
[![npm](https://img.shields.io/npm/v/typeul?logo=npm)](https://www.npmjs.com/package/typeul)
[![License: MIT](https://img.shields.io/badge/license-MIT-14B8A6)](LICENSE)

Practice typing—or take a quick terminal break when vibe coding gets dull.

Typeul is an offline-first Korean and English typing tutor for the terminal. It combines six
practice modes, local progress, weak-key analysis, and source-aware practice material without an
account, telemetry, or cloud storage.

## Start in one minute

When both registry badges above show a published version:

```bash
npm install -g typeul
typeul
```

The npm package installs the matching prebuilt binary for macOS, Linux, or Windows on x64 and
arm64. Rust is not required for this path.

<details>
<summary>Install with Cargo</summary>

```bash
cargo install typeul
typeul
```

</details>

Until the first release—or to try current `main`—run it from source:

```bash
git clone https://github.com/baba9811/typeul.git
cd typeul
cargo run --release --
```

Use a UTF-8 interactive terminal. The recommended minimum size is 80×24.

## Why Typeul

- **Korean and English:** practice Dubeolsik Korean keys or English QWERTY in one app.
- **Six useful modes:** Keys, Quick, Words, Sentence, Long Text, and Typing Test.
- **Honest metrics:** active-time speed, attempt-based accuracy, rolling best speed, and weak keys.
- **Local by default:** settings and aggregate session history stay on your machine.
- **Bring your own text:** open a UTF-8 file, pipe stdin on Unix, or install a reusable content pack.
- **Auditable material:** bundled items keep their source, modification status, and license metadata.

## Practice modes

| Mode | Best for |
| --- | --- |
| Keys | Learning Dubeolsik or QWERTY progressively |
| Quick | A short timed or item-count break |
| Words | Difficulty-aware and adaptive vocabulary practice |
| Sentence | Immediate speed and accuracy feedback per sentence |
| Long Text | Paragraph progress, provenance, and rolling best speed |
| Typing Test | An uninterrupted timed test with a relative grade |

Choose a mode from Home, adjust its options, and press `Enter`. Typing Test cannot be paused.

## Essential controls

| Key | Action |
| --- | --- |
| `Tab`, `Shift+Tab`, `↑`, `↓`, `j`, `k` | Move focus |
| `Enter` | Select, start, or commit a newline |
| `Esc`, `Ctrl+P` | Go back or pause/resume non-Test practice |
| `q`, `Ctrl+C` | Quit; `q` also confirms an early finish |
| `Backspace` | Correct within the current item |
| `r`, `n` | Retry or start the next supported practice from Result |
| `?` | Open contextual help |

Paste is ignored during scored practice. Run `typeul --help` for the complete command reference.

## Practice your own text

```bash
typeul notes.txt
typeul practice notes.txt
cat notes.txt | typeul  # Unix with a controlling terminal
```

Custom text stays in memory and is never copied into a session record. For reusable word,
sentence, quote, or long-text collections, see the
[content-pack guide](https://github.com/baba9811/typeul/blob/main/docs/content-packs.md).

Useful commands:

```bash
typeul stats
typeul history
typeul paths
typeul themes
typeul licenses
typeul update
```

## Local data and privacy

`typeul paths` prints the exact config, session, content, theme, and cache locations for the current
platform. Set `TYPEUL_HOME=/path` to place all of them under one root.

Session files contain aggregate timing, accuracy, speed, error, backspace, mode, and intended-key
counts. They never contain the target text, what you typed, custom filenames, per-keystroke
timestamps, accounts, or network identifiers. Corrupt or unsupported user files are preserved and
reported instead of silently replaced.

## Content and licenses

Bundled practice text uses directly typable ASCII and, for Korean, modern Hangul syllables.
Source-only typography such as circled paragraph numbers is transcribed to keyboard characters and
marked as modified in its provenance. User-supplied text is not subject to that bundled-data rule.

Typeul is MIT licensed. Project-authored practice data is CC0 1.0; other bundled material keeps its
declared public-domain, Creative Commons, or MIT terms. Run `typeul licenses` for the complete
offline notices, or read [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Development

```bash
git clone https://github.com/baba9811/typeul.git
cd typeul
cargo run --
make test
```

Typeul pins Rust 1.88. The full gate also uses Node.js and the policy tools documented by the CI
workflow. Maintainers should use the
[release guide](https://github.com/baba9811/typeul/blob/main/docs/releasing.md); security issues
belong in [GitHub Security Advisories](https://github.com/baba9811/typeul/security/advisories/new),
not public issues.

## License

[MIT](LICENSE) © Typeul contributors.
