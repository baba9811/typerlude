import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  parseBase,
  parseCc0,
  parseDetailed,
  renderPacks,
  selectedRows,
} from "./import-tatoeba.mjs";

test("parses and pins a detailed sentence row", () => {
  const row = parseDetailed(
    "42\tkor\t정확히 씁니다.\twriter\t2020-01-01\t2020-01-02",
  );

  assert.equal(selectedRows([row], [42])[0].author, "writer");
  assert.deepEqual(parseBase("42\t0"), { id: "42", base: "0" });
  assert.deepEqual(parseCc0("43\teng\tType this line.\t2020-01-02"), {
    id: "43",
    language: "eng",
    text: "Type this line.",
    modified: "2020-01-02",
  });
});

test("rejects a selected id absent from the frozen export", () => {
  assert.throws(() => selectedRows([], [42]), /missing: 42/);
});

test("renders exact reviewed splits with stable sentence provenance", () => {
  const fixture = corpusFixture();
  const first = renderPacks(fixture);
  const second = renderPacks(fixture);

  assert.deepEqual(first, second);
  assert.equal((first.ko.match(/^\[\[items\]\]$/gm) ?? []).length, 120);
  assert.equal((first.en.match(/^\[\[items\]\]$/gm) ?? []).length, 120);
  assert.equal((first.ko.match(/^kind = "sentence"$/gm) ?? []).length, 100);
  assert.equal((first.ko.match(/^kind = "quote"$/gm) ?? []).length, 20);
  assert.match(first.ko, /author = "writer1"/);
  assert.match(first.ko, /source_id = "tatoeba:1"/);
  assert.match(first.ko, /source_url = "https:\/\/tatoeba\.org\/en\/sentences\/show\/1"/);
  assert.match(first.ko, /license = "CC-BY-2\.0-FR"/);
  assert.match(first.en, /author = "Tatoeba CC0 contributors"/);
  assert.match(first.en, /license = "CC0-1\.0"/);
});

test("rejects decompressed input bytes that do not match the frozen snapshot", () => {
  const fixture = corpusFixture();
  fixture.korDetailed = Buffer.from(
    fixture.korDetailed.replace(
      "안전한 연습 문장 1입니다.",
      "새로운 연습 문장 1입니다.",
    ),
    "utf8",
  );

  assert.throws(
    () => renderPacks(fixture),
    /decompressed input does not match snapshot for kor_detailed/,
  );

  const wrongSize = corpusFixture();
  wrongSize.snapshot.exports[0].decompressed_bytes += 1;
  assert.throws(
    () => renderPacks(wrongSize),
    /decompressed input does not match snapshot for kor_detailed/,
  );
});

test("requires exactly one snapshot entry for each supported export", () => {
  const duplicate = corpusFixture();
  duplicate.snapshot.exports.push({ ...duplicate.snapshot.exports[0] });
  assert.throws(
    () => renderPacks(duplicate),
    /snapshot must contain exactly the three unique export keys/,
  );

  const missing = corpusFixture();
  missing.snapshot.exports.pop();
  assert.throws(
    () => renderPacks(missing),
    /snapshot must contain exactly the three unique export keys/,
  );

  const extra = corpusFixture();
  extra.snapshot.exports.push({
    ...extra.snapshot.exports[0],
    key: "unexpected",
  });
  assert.throws(
    () => renderPacks(extra),
    /snapshot must contain exactly the three unique export keys/,
  );
});

test("requires the complete reviewed method and grapheme range", () => {
  const missingMethod = corpusFixture();
  delete missingMethod.selection.review.method;
  assert.throws(
    () => renderPacks(missingMethod),
    /selection review metadata is incomplete/,
  );

  const changedMethod = corpusFixture();
  changedMethod.selection.review.method = "Only some rows were inspected.";
  assert.throws(
    () => renderPacks(changedMethod),
    /selection review metadata is incomplete/,
  );

  const changedRange = corpusFixture();
  changedRange.selection.review.grapheme_range = [1, 120];
  assert.throws(
    () => renderPacks(changedRange),
    /selection review metadata is incomplete/,
  );
});

test("rejects any ordered selection tuple change without a new review digest", () => {
  const mutations = [
    (selection) => {
      [selection.ko[0], selection.ko[1]] = [selection.ko[1], selection.ko[0]];
    },
    (selection) => {
      selection.ko[0].id = 999;
    },
    (selection) => {
      selection.ko[0].kind = "quote";
      selection.ko[100].kind = "sentence";
    },
    (selection) => {
      selection.ko[0].author = "someone-else";
    },
  ];

  for (const mutate of mutations) {
    const fixture = corpusFixture();
    mutate(fixture.selection);
    assert.throws(
      () => renderPacks(fixture),
      /selection digest does not match reviewed tuples/,
    );
  }
});

test("the repository tracks no raw Tatoeba TSV or BZ2 export", () => {
  const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
  const tracked = execFileSync("git", ["ls-files", "-z"], {
    cwd: repositoryRoot,
    encoding: "utf8",
  })
    .split("\0")
    .filter(Boolean);
  const rawExports = tracked.filter((path) => /\.(?:tsv|bz2)$/u.test(path));

  assert.deepEqual(rawExports, []);
});

test("rejects derived, unattributed, non-NFC, and unsafe selected rows", () => {
  const derived = corpusFixture();
  derived.korBase = derived.korBase.replace("1\t0", "1\t9");
  refreshInputMetadata(derived, "korBase", "kor_base");
  assert.throws(() => renderPacks(derived), /Korean sentence 1 must have base 0/);

  const unattributed = corpusFixture();
  unattributed.korDetailed = unattributed.korDetailed.replace("\twriter1\t", "\t\t");
  refreshInputMetadata(unattributed, "korDetailed", "kor_detailed");
  assert.throws(() => renderPacks(unattributed), /Korean sentence 1 has no contributor/);

  const nonNfc = corpusFixture();
  nonNfc.engCc0 = nonNfc.engCc0.replace("Practice sentence 121.", "Cafe\u0301 practice line.");
  refreshInputMetadata(nonNfc, "engCc0", "eng_cc0");
  assert.throws(() => renderPacks(nonNfc), /sentence 121 is not NFC/);

  const unsafe = corpusFixture();
  unsafe.engCc0 = unsafe.engCc0.replace(
    "Practice sentence 121.",
    "Visit https://example.com now.",
  );
  refreshInputMetadata(unsafe, "engCc0", "eng_cc0");
  assert.throws(() => renderPacks(unsafe), /sentence 121 contains a URL/);

  const unattributedCc0 = corpusFixture();
  delete unattributedCc0.selection.en[0].author;
  assert.throws(
    () => renderPacks(unattributedCc0),
    /en selection 121 has no contributor/,
  );

  const mismatchedContributor = corpusFixture();
  mismatchedContributor.selection.ko[0].author = "someone-else";
  refreshSelectionDigest(mismatchedContributor.selection);
  assert.throws(
    () => renderPacks(mismatchedContributor),
    /Korean sentence 1 contributor does not match the frozen selection/,
  );

  const unreviewed = corpusFixture();
  delete unreviewed.selection.review;
  assert.throws(() => renderPacks(unreviewed), /selection review metadata is incomplete/);
});

function corpusFixture() {
  const ko = [];
  const base = [];
  const en = [];
  const koSelection = [];
  const enSelection = [];

  for (let index = 1; index <= 120; index += 1) {
    ko.push(
      `${index}\tkor\t안전한 연습 문장 ${index}입니다.\twriter${index}\t2020-01-01\t2020-01-02`,
    );
    base.push(`${index}\t0`);
    en.push(`${index + 120}\teng\tPractice sentence ${index + 120}.\t2020-01-02`);
    koSelection.push({
      id: index,
      kind: index <= 100 ? "sentence" : "quote",
      author: `writer${index}`,
    });
    enSelection.push({
      id: index + 120,
      kind: index <= 100 ? "sentence" : "quote",
      author: "Tatoeba CC0 contributors",
    });
  }

  const fixture = {
    korDetailed: ko.join("\n"),
    korBase: base.join("\n"),
    engCc0: en.join("\n"),
    selection: {
      schema_version: 1,
      reviewed_at: "2026-08-07",
      review: {
        reviewer: "Typeul corpus review",
        method:
          "Each selected TSV row was inspected individually for natural language quality, general-audience safety, privacy, and typing value after deterministic eligibility filtering.",
        criteria: ["language_quality", "general_audience_safety", "privacy", "typing_value"],
        normalization_changed: false,
        korean_original_only: true,
        english_explicit_cc0_only: true,
        grapheme_range: [8, 120],
      },
      ko: koSelection,
      en: enSelection,
    },
    snapshot: {
      retrieved_at: "2026-08-07",
      exports: [
        {
          key: "kor_detailed",
          url: "https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_detailed.tsv.bz2",
          bytes: 1,
          sha256: "a".repeat(64),
        },
        {
          key: "kor_base",
          url: "https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_base.tsv.bz2",
          bytes: 1,
          sha256: "b".repeat(64),
        },
        {
          key: "eng_cc0",
          url: "https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences_CC0.tsv.bz2",
          bytes: 1,
          sha256: "c".repeat(64),
        },
      ],
    },
  };

  refreshInputMetadata(fixture, "korDetailed", "kor_detailed");
  refreshInputMetadata(fixture, "korBase", "kor_base");
  refreshInputMetadata(fixture, "engCc0", "eng_cc0");
  refreshSelectionDigest(fixture.selection);
  return fixture;
}

function refreshInputMetadata(fixture, inputKey, exportKey) {
  const bytes = Buffer.from(fixture[inputKey], "utf8");
  const metadata = fixture.snapshot.exports.find(({ key }) => key === exportKey);
  metadata.decompressed_bytes = bytes.byteLength;
  metadata.decompressed_sha256 = sha256(bytes);
}

function canonicalSelectionTuples(selection) {
  return JSON.stringify({
    ko: selection.ko.map(({ id, kind, author }) => [id, kind, author]),
    en: selection.en.map(({ id, kind, author }) => [id, kind, author]),
  });
}

function refreshSelectionDigest(selection) {
  selection.review.selection_sha256 = sha256(canonicalSelectionTuples(selection));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
