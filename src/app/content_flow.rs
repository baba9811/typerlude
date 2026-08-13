use super::{App, ContentPackSummary, ContentProvenance};
use crate::{
    content::{
        ContentCatalog, ContentKind, ResolvedItem, disable_user_pack, parse_pack, read_pack_bytes,
        validate_pack,
    },
    diagnostic::format_content_error,
};
use std::{collections::BTreeMap, fs, path::Path};

impl App {
    pub fn content_packs(&self) -> &[ContentPackSummary] {
        &self.content_pack_summaries
    }

    pub fn selected_content_pack(&self) -> Option<&str> {
        self.selected_content_pack.as_deref()
    }

    pub fn content_detail_pack(&self) -> Option<&ContentPackSummary> {
        self.selected_content_pack
            .as_deref()
            .and_then(|id| {
                self.content_pack_summaries
                    .iter()
                    .find(|pack| pack.id == id)
            })
            .or_else(|| self.content_pack_summaries.first())
    }

    pub const fn content_disable_confirmation(&self) -> bool {
        self.content_disable_confirmation
    }

    pub(super) fn disable_selected_content(&mut self) {
        let Some(id) = self.selected_content_pack.clone() else {
            return;
        };
        let Some(pack) = self.content_packs().iter().find(|pack| pack.id == id) else {
            return;
        };
        if pack.built_in {
            self.warnings
                .push(format!("content: built-in pack {id:?} cannot be disabled"));
            return;
        }
        if !pack.enabled {
            self.warnings
                .push(format!("content: user pack {id:?} is already disabled"));
            return;
        }
        if !self.content_disable_confirmation {
            self.content_disable_confirmation = true;
            return;
        }
        let mut mutation_warnings = Vec::new();
        let result = disable_user_pack(&self.paths, &id, &mut mutation_warnings);
        self.warnings.extend(
            mutation_warnings
                .iter()
                .map(|warning| format!("content: {}", format_content_error(warning))),
        );
        match result {
            Ok(catalog) => {
                self.content_pack_summaries = collect_content_packs(&catalog, &self.paths.content);
                self.content = catalog;
                self.selected_content_pack = None;
                self.content_disable_confirmation = false;
                self.escape();
            }
            Err(error) => {
                self.warnings.push(format!("content: {error:#}"));
                self.content_disable_confirmation = false;
            }
        }
    }
}

pub(super) fn collect_content_packs(
    catalog: &ContentCatalog,
    content_root: &Path,
) -> Vec<ContentPackSummary> {
    let mut packs = BTreeMap::<String, ContentPackSummary>::new();
    for item in catalog.items() {
        add_pack_item(
            &mut packs,
            item,
            true,
            catalog.active_user_path(&item.pack_id).is_none(),
        );
    }
    for pack in packs.values_mut() {
        if let Some(source) = catalog.pack_source(&pack.id) {
            if !pack.licenses.contains(&source.license) {
                pack.licenses.push(source.license.clone());
            }
            pack.provenance.push(ContentProvenance {
                item_id: None,
                source: source.clone(),
            });
        }
    }

    let disabled = content_root.join("disabled");
    let mut entries = match fs::symlink_metadata(&disabled) {
        Ok(metadata) if metadata.file_type().is_dir() => fs::read_dir(disabled)
            .map(|entries| entries.flatten().collect::<Vec<_>>())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    entries.sort_unstable_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml")
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        let Ok(bytes) = read_pack_bytes(&path) else {
            continue;
        };
        let Ok(source) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(pack) = parse_pack(source) else {
            continue;
        };
        if !validate_pack(&pack).is_empty() || packs.contains_key(&pack.id) {
            continue;
        }
        let pack_id = pack.id.clone();
        let pack_source = pack.source.clone();
        let Ok(items) = pack.resolve_items() else {
            continue;
        };
        for item in items {
            add_pack_item(&mut packs, &item, false, false);
        }
        if let Some(pack) = packs.get_mut(&pack_id) {
            if !pack.licenses.contains(&pack_source.license) {
                pack.licenses.push(pack_source.license.clone());
            }
            pack.provenance.push(ContentProvenance {
                item_id: None,
                source: pack_source,
            });
        }
    }

    for pack in packs.values_mut() {
        pack.licenses.sort_unstable();
        pack.kinds
            .sort_unstable_by_key(|kind| content_kind_order(*kind));
    }
    packs.into_values().collect()
}

fn add_pack_item(
    packs: &mut BTreeMap<String, ContentPackSummary>,
    item: &ResolvedItem,
    enabled: bool,
    built_in: bool,
) {
    let pack = packs
        .entry(item.pack_id.clone())
        .or_insert_with(|| ContentPackSummary {
            id: item.pack_id.clone(),
            sample_item_id: item.id.clone(),
            provenance: Vec::new(),
            language: item.language,
            items: 0,
            licenses: Vec::new(),
            kinds: Vec::new(),
            enabled,
            built_in,
        });
    pack.items += 1;
    if !pack
        .provenance
        .iter()
        .any(|value| value.item_id.is_some() && value.source == item.source)
    {
        pack.provenance.push(ContentProvenance {
            item_id: Some(item.id.clone()),
            source: item.source.clone(),
        });
    }
    if !pack
        .licenses
        .iter()
        .any(|value| value == &item.source.license)
    {
        pack.licenses.push(item.source.license.clone());
    }
    if !pack.kinds.contains(&item.kind) {
        pack.kinds.push(item.kind);
    }
}

const fn content_kind_order(kind: ContentKind) -> u8 {
    match kind {
        ContentKind::Word => 0,
        ContentKind::Sentence => 1,
        ContentKind::Quote => 2,
        ContentKind::Text => 3,
    }
}
