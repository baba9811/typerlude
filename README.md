# Typerlude

A typing interlude for your terminal.

[English](README.md) | [한국어](README.ko.md)

Typerlude is an offline-first Korean and English typing tutor for the terminal. Practice keys,
words, sentences, long text, and timed tests without an account, telemetry, or cloud storage.

## Install

Registry availability is not yet verified. Once the packages are published:

```bash
npm install -g typerlude
typerlude
```

```bash
cargo install typerlude
typerlude
```

Until then, run the current source:

```bash
git clone https://github.com/baba9811/typerlude.git
cd typerlude
cargo run --release --
```

Use a UTF-8 interactive terminal; 80×24 is the recommended minimum size.

## Use

Choose a mode on the home screen, adjust its options, then press `Enter`. `Tab`, arrow keys,
`j`, and `k` move focus; `Esc` goes back or pauses supported practice; `q` quits. Paste is
ignored during scored practice.

```bash
typerlude --help
typerlude notes.txt
typerlude stats
typerlude history
typerlude paths
typerlude themes
typerlude licenses
```

User text stays in memory and is not copied into session history. Local session records contain
only aggregate practice metrics and intended-key counts.

## More

- [Content-pack guide](docs/content-packs.md) / [콘텐츠 팩 안내](docs/content-packs.ko.md)
- [Release guide](docs/releasing.md)
- [Security policy](SECURITY.md)
- [License and third-party notices](LICENSE) / [data rights](THIRD_PARTY_NOTICES.md)

## Develop

```bash
git clone https://github.com/baba9811/typerlude.git
cd typerlude
cargo run --
make test
```

Typerlude pins Rust 1.88.0. The complete gate also uses Node.js and the policy tools documented by
CI.
