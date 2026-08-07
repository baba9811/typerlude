#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const EXPORT_URLS = {
  kor_detailed:
    "https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_detailed.tsv.bz2",
  kor_base:
    "https://downloads.tatoeba.org/exports/per_language/kor/kor_sentences_base.tsv.bz2",
  eng_cc0:
    "https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences_CC0.tsv.bz2",
};

const LICENSES = {
  ko: {
    name: "CC-BY-2.0-FR",
    url: "https://creativecommons.org/licenses/by/2.0/fr/",
  },
  en: {
    name: "CC0-1.0",
    url: "https://creativecommons.org/publicdomain/zero/1.0/",
  },
};

export function parseDetailed(line) {
  const fields = line.split("\t");
  if (fields.length !== 6) throw new Error("invalid detailed row");
  const [id, language, text, author, added, modified] = fields;
  if (!/^\d+$/.test(id) || !language || !text) {
    throw new Error("invalid detailed row");
  }
  return { id, language, text, author, added, modified };
}

export function parseBase(line) {
  const fields = line.split("\t");
  if (fields.length !== 2) throw new Error("invalid base row");
  const [id, base] = fields;
  if (!/^\d+$/.test(id) || !/^(0|[1-9]\d*|\\N)$/.test(base)) {
    throw new Error("invalid base row");
  }
  return { id, base };
}

export function parseCc0(line) {
  const fields = line.split("\t");
  if (fields.length !== 4) throw new Error("invalid CC0 row");
  const [id, language, text, modified] = fields;
  if (!/^\d+$/.test(id) || !language || !text) throw new Error("invalid CC0 row");
  return { id, language, text, modified };
}

export function selectedRows(rows, selectedIds) {
  const byId = new Map();
  for (const row of rows) {
    if (byId.has(row.id)) throw new Error(`duplicate sentence in export: ${row.id}`);
    byId.set(row.id, row);
  }
  return selectedIds.map((id) => {
    const row = byId.get(String(id));
    if (!row) throw new Error(`selected sentence missing: ${id}`);
    return row;
  });
}

export function renderPacks({ korDetailed, korBase, engCc0, selection, snapshot }) {
  validateSnapshot(snapshot);
  validateSelection(selection, snapshot.retrieved_at);

  const koSelections = selection.ko;
  const enSelections = selection.en;
  const koRows = selectedRows(
    parseLines(korDetailed, parseDetailed),
    koSelections.map(({ id }) => id),
  );
  const koBases = selectedRows(
    parseLines(korBase, parseBase),
    koSelections.map(({ id }) => id),
  );
  const enRows = selectedRows(
    parseLines(engCc0, parseCc0),
    enSelections.map(({ id }) => id),
  );

  koRows.forEach((row, index) => {
    if (row.language !== "kor") throw new Error(`sentence ${row.id} is not Korean`);
    if (!row.author || row.author === "\\N") {
      throw new Error(`Korean sentence ${row.id} has no contributor`);
    }
    if (row.author !== koSelections[index].author) {
      throw new Error(
        `Korean sentence ${row.id} contributor does not match the frozen selection`,
      );
    }
    if (koBases[index].base !== "0") {
      throw new Error(`Korean sentence ${row.id} must have base 0`);
    }
    validateText(row);
  });
  enRows.forEach((row) => {
    if (row.language !== "eng") throw new Error(`sentence ${row.id} is not English`);
    validateText(row);
  });

  return {
    ko: renderPack({
      id: "ko-sentences",
      title: "Korean Sentences",
      language: "ko",
      rows: koRows,
      selected: koSelections,
      snapshot,
      exportKey: "kor_detailed",
      license: LICENSES.ko,
    }),
    en: renderPack({
      id: "en-sentences",
      title: "English Sentences",
      language: "en",
      rows: enRows,
      selected: enSelections,
      snapshot,
      exportKey: "eng_cc0",
      license: LICENSES.en,
    }),
  };
}

function parseLines(source, parser) {
  return source
    .split("\n")
    .map((line) => line.replace(/\r$/, ""))
    .filter(Boolean)
    .map(parser);
}

function validateSnapshot(snapshot) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(snapshot?.retrieved_at ?? "")) {
    throw new Error("snapshot has invalid retrieval date");
  }
  const exportsByKey = new Map((snapshot.exports ?? []).map((item) => [item.key, item]));
  for (const [key, url] of Object.entries(EXPORT_URLS)) {
    const item = exportsByKey.get(key);
    if (
      item?.url !== url ||
      !Number.isSafeInteger(item.bytes) ||
      item.bytes <= 0 ||
      !/^[0-9a-f]{64}$/.test(item.sha256 ?? "")
    ) {
      throw new Error(`invalid snapshot metadata for ${key}`);
    }
  }
}

function validateSelection(selection, retrievedAt) {
  if (selection?.schema_version !== 1 || selection.reviewed_at !== retrievedAt) {
    throw new Error("selection and snapshot metadata do not match");
  }
  const expectedCriteria = [
    "language_quality",
    "general_audience_safety",
    "privacy",
    "typing_value",
  ];
  const review = selection.review;
  if (
    typeof review?.reviewer !== "string" ||
    !review.reviewer.trim() ||
    JSON.stringify(review.criteria) !== JSON.stringify(expectedCriteria) ||
    review.normalization_changed !== false ||
    review.korean_original_only !== true ||
    review.english_explicit_cc0_only !== true
  ) {
    throw new Error("selection review metadata is incomplete");
  }
  for (const language of ["ko", "en"]) {
    const items = selection[language];
    if (!Array.isArray(items) || items.length !== 120) {
      throw new Error(`${language} selection must contain 120 items`);
    }
    const ids = new Set();
    let sentences = 0;
    let quotes = 0;
    for (const item of items) {
      if (!Number.isSafeInteger(item.id) || item.id <= 0 || ids.has(item.id)) {
        throw new Error(`${language} selection has an invalid or duplicate ID`);
      }
      if (typeof item.author !== "string" || !item.author.trim()) {
        throw new Error(`${language} selection ${item.id} has no contributor`);
      }
      if (language === "en" && item.author !== "Tatoeba CC0 contributors") {
        throw new Error(`en selection ${item.id} has an invalid contributor label`);
      }
      ids.add(item.id);
      if (item.kind === "sentence") sentences += 1;
      else if (item.kind === "quote") quotes += 1;
      else throw new Error(`${language} selection has an invalid kind`);
    }
    if (sentences !== 100 || quotes !== 20) {
      throw new Error(`${language} selection must contain 100 sentences and 20 quotes`);
    }
  }
}

function validateText(row) {
  if (row.text !== row.text.normalize("NFC")) {
    throw new Error(`sentence ${row.id} is not NFC`);
  }
  if (row.text !== row.text.trim()) throw new Error(`sentence ${row.id} has outer whitespace`);
  if (/\p{Cc}|\p{Cf}/u.test(row.text)) {
    throw new Error(`sentence ${row.id} contains a control character`);
  }
  if (/https?:\/\/|www\./iu.test(row.text)) {
    throw new Error(`sentence ${row.id} contains a URL`);
  }
  const length = [...new Intl.Segmenter("und", { granularity: "grapheme" }).segment(row.text)]
    .length;
  if (length < 8 || length > 120) {
    throw new Error(`sentence ${row.id} must contain 8 to 120 graphemes`);
  }
}

function renderPack({ id, title, language, rows, selected, snapshot, exportKey, license }) {
  const exportMeta = snapshot.exports.find(({ key }) => key === exportKey);
  const lines = [
    "schema_version = 1",
    `id = ${tomlString(id)}`,
    `title = ${tomlString(title)}`,
    `language = ${tomlString(language)}`,
    "",
    "[source]",
    'author = "Tatoeba contributors"',
    `source_id = ${tomlString(`tatoeba-${exportKey}-${exportMeta.sha256}`)}`,
    `source_url = ${tomlString(exportMeta.url)}`,
    `license = ${tomlString(license.name)}`,
    `license_url = ${tomlString(license.url)}`,
    "modified = false",
    `retrieved_at = ${tomlString(snapshot.retrieved_at)}`,
  ];

  rows.forEach((row, index) => {
    const item = selected[index];
    lines.push(
      "",
      "[[items]]",
      `id = ${tomlString(`${language}-tatoeba-${row.id}`)}`,
      `kind = ${tomlString(item.kind)}`,
      `text = ${tomlString(row.text)}`,
      `tags = [${tomlString("tatoeba")}, ${tomlString(item.kind === "quote" ? "quick" : "sentence")}]`,
      "[items.source]",
      `author = ${tomlString(language === "ko" ? row.author : item.author)}`,
      `source_id = ${tomlString(`tatoeba:${row.id}`)}`,
      `source_url = ${tomlString(`https://tatoeba.org/en/sentences/show/${row.id}`)}`,
      `license = ${tomlString(license.name)}`,
      `license_url = ${tomlString(license.url)}`,
      "modified = false",
      `retrieved_at = ${tomlString(snapshot.retrieved_at)}`,
    );
  });

  return `${lines.join("\n")}\n`;
}

function tomlString(value) {
  return JSON.stringify(value);
}

function main(argv) {
  const flags = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined || value.startsWith("--")) {
      throw new Error("arguments must be --name value pairs");
    }
    if (flags.has(flag)) throw new Error(`duplicate argument: ${flag}`);
    flags.set(flag, value);
  }
  const required = ["--kor-detailed", "--kor-base", "--eng-cc0", "--selection", "--snapshot"];
  for (const flag of required) {
    if (!flags.has(flag)) throw new Error(`missing argument: ${flag}`);
  }
  if (flags.size !== required.length) throw new Error("unknown argument");

  const packs = renderPacks({
    korDetailed: readFileSync(flags.get("--kor-detailed"), "utf8"),
    korBase: readFileSync(flags.get("--kor-base"), "utf8"),
    engCc0: readFileSync(flags.get("--eng-cc0"), "utf8"),
    selection: JSON.parse(readFileSync(flags.get("--selection"), "utf8")),
    snapshot: JSON.parse(readFileSync(flags.get("--snapshot"), "utf8")),
  });
  writeFileSync("assets/content/ko-sentences.toml", packs.ko);
  writeFileSync("assets/content/en-sentences.toml", packs.en);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
