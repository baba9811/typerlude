mod catalog;
mod mutation;
mod validation;

pub(crate) use catalog::read_pack_bytes;
pub use validation::validate_pack;

use crate::model::Language;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

pub(crate) use mutation::{MutationLock, disable_user_pack};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Word,
    Sentence,
    Quote,
    Text,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceMeta {
    pub author: String,
    pub source_id: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
    pub modified: bool,
    pub retrieved_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContentItem {
    pub id: String,
    pub kind: ContentKind,
    pub text: String,
    pub difficulty: Option<u8>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub source: Option<SourceMeta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContentPack {
    pub schema_version: u16,
    pub id: String,
    pub title: String,
    pub language: Language,
    pub source: SourceMeta,
    pub items: Vec<ContentItem>,
}

#[derive(Clone, Debug)]
pub struct ResolvedItem {
    pub pack_id: String,
    pub id: String,
    pub language: Language,
    pub kind: ContentKind,
    pub text: String,
    pub difficulty: Option<u8>,
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub source: SourceMeta,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentError {
    pub pack_id: String,
    pub item_id: Option<String>,
    pub field: String,
    pub message: String,
}

#[derive(Default)]
pub struct ContentCatalog {
    items: Vec<ResolvedItem>,
    pack_ids: HashSet<String>,
    item_ids: HashSet<String>,
    normalized_texts: HashSet<(Language, ContentKind, String)>,
    pack_sources: HashMap<String, SourceMeta>,
    user_pack_paths: HashMap<String, PathBuf>,
}

pub struct CatalogLoad {
    pub catalog: ContentCatalog,
    pub warnings: Vec<ContentError>,
}

pub(crate) const MAX_CONTENT_BYTES: usize = 8 * 1024 * 1024;

pub fn parse_pack(source: &str) -> Result<ContentPack> {
    toml::from_str(source).context("invalid content pack TOML")
}

#[cfg(test)]
mod tests;
