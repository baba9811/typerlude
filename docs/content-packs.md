# Content packs

[English](content-packs.md) | [한국어](content-packs.ko.md)

Use a content pack when you want a reusable collection with searchable metadata and explicit
provenance. For one-off practice, `typerlude FILE` is simpler.

## Add a pack

Validate before installing. Typerlude uses the same schema, NFC, duplicate, provenance, and license
checks at validation and startup, and never overwrites an existing pack.

```bash
typerlude content validate my-pack.toml
typerlude content add my-pack.toml
typerlude content list
```

Minimal example:

```toml
schema_version = 1
id = "my-pack"
title = "My practice pack"
language = "en" # en | ko

[source]
author = "Your Name"
source_id = "my-pack-v1"
source_url = "https://example.com/my-pack"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-11"

[[items]]
id = "my-pack-word-one"
kind = "word" # word | sentence | quote | text
text = "example"
difficulty = 1 # optional, 1..3
tags = ["custom"]
```

Pack and item IDs must be unique. Installable pack IDs use ASCII letters, digits, `-`, and `_`.
Text must be nonblank NFC UTF-8 without terminal control characters; duplicates are checked within
the same language and content kind.

Supported declared licenses are `CC0-1.0`, `CC-BY-2.0-FR`, `CC-BY-4.0`, `KOGL-0`, `KOGL-1.0`,
and `LicenseRef-Public-Domain`.

Attribution alone does not grant redistribution rights. Add only material you own, public-domain
material, or material whose license permits redistribution in npm and crates.io packages, and
follow every license condition.

## Disable a pack

```bash
typerlude content disable my-pack
```

Only user packs can be disabled. Typerlude moves the accepted file into `content/disabled/` without
overwriting an existing entry. Run `typerlude paths` to find that directory.
