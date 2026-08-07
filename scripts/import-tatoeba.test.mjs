import assert from "node:assert/strict";
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

test("rejects derived, unattributed, non-NFC, and unsafe selected rows", () => {
  const derived = corpusFixture();
  derived.korBase = derived.korBase.replace("1\t0", "1\t9");
  assert.throws(() => renderPacks(derived), /Korean sentence 1 must have base 0/);

  const unattributed = corpusFixture();
  unattributed.korDetailed = unattributed.korDetailed.replace("\twriter1\t", "\t\t");
  assert.throws(() => renderPacks(unattributed), /Korean sentence 1 has no contributor/);

  const nonNfc = corpusFixture();
  nonNfc.engCc0 = nonNfc.engCc0.replace("Practice sentence 121.", "Cafe\u0301 practice line.");
  assert.throws(() => renderPacks(nonNfc), /sentence 121 is not NFC/);

  const unsafe = corpusFixture();
  unsafe.engCc0 = unsafe.engCc0.replace(
    "Practice sentence 121.",
    "Visit https://example.com now.",
  );
  assert.throws(() => renderPacks(unsafe), /sentence 121 contains a URL/);

  const unattributedCc0 = corpusFixture();
  delete unattributedCc0.selection.en[0].author;
  assert.throws(
    () => renderPacks(unattributedCc0),
    /en selection 121 has no contributor/,
  );

  const mismatchedContributor = corpusFixture();
  mismatchedContributor.selection.ko[0].author = "someone-else";
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

  return {
    korDetailed: ko.join("\n"),
    korBase: base.join("\n"),
    engCc0: en.join("\n"),
    selection: {
      schema_version: 1,
      reviewed_at: "2026-08-07",
      review: {
        reviewer: "Typeul corpus review",
        criteria: ["language_quality", "general_audience_safety", "privacy", "typing_value"],
        normalization_changed: false,
        korean_original_only: true,
        english_explicit_cc0_only: true,
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
}
