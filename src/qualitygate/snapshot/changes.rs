use super::File;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub kind: String,
    pub old_path: Option<String>,
    pub added_lines: BTreeSet<usize>,
}

pub(super) fn compare(
    base: &BTreeMap<String, File>,
    head: &BTreeMap<String, File>,
) -> BTreeMap<String, Change> {
    let mut changes = BTreeMap::new();
    for (path, file) in head {
        let old = base.get(path);
        if old.is_some_and(|old| old.bytes == file.bytes && old.executable == file.executable) {
            continue;
        }
        let moved = if old.is_none() {
            base.iter()
                .find(|(name, old)| !head.contains_key(*name) && old.bytes == file.bytes)
        } else {
            None
        };
        let old_bytes = old.map(|file| file.bytes.as_slice()).unwrap_or_default();
        let old_text = String::from_utf8_lossy(old_bytes);
        let text = String::from_utf8_lossy(&file.bytes);
        let added_lines = if moved.is_some() {
            BTreeSet::new()
        } else {
            TextDiff::from_lines(old_text.as_ref(), text.as_ref())
                .iter_all_changes()
                .filter(|change| change.tag() == ChangeTag::Insert)
                .filter_map(|change| change.new_index().map(|index| index + 1))
                .collect()
        };
        changes.insert(
            path.clone(),
            Change {
                kind: if moved.is_some() {
                    "renamed"
                } else if old.is_some() {
                    "modified"
                } else {
                    "added"
                }
                .into(),
                old_path: moved
                    .map(|(name, _)| name.clone())
                    .or_else(|| old.map(|_| path.clone())),
                added_lines,
            },
        );
    }
    for path in base.keys().filter(|path| !head.contains_key(*path)) {
        if changes
            .values()
            .any(|change| change.old_path.as_ref() == Some(path))
        {
            continue;
        }
        changes.insert(
            path.clone(),
            Change {
                kind: "deleted".into(),
                old_path: Some(path.clone()),
                added_lines: BTreeSet::new(),
            },
        );
    }
    changes
}
