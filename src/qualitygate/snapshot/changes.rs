use super::File;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub kind: String,
    pub old_path: Option<String>,
    pub added_lines: BTreeSet<usize>,
    #[serde(default)]
    pub removed_lines: BTreeSet<usize>,
}

pub fn compare(
    base: &BTreeMap<String, File>,
    head: &BTreeMap<String, File>,
) -> BTreeMap<String, Change> {
    compare_until(base, head, None).expect("comparison without a deadline cannot time out")
}

pub(super) fn check_deadline(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        bail!("Snapshot acquisition timed out during change mapping or hashing");
    }
    Ok(())
}

pub(super) fn compare_until(
    base: &BTreeMap<String, File>,
    head: &BTreeMap<String, File>,
    deadline: Option<Instant>,
) -> Result<BTreeMap<String, Change>> {
    check_deadline(deadline)?;
    let mut changes = BTreeMap::new();
    // Index only removed content once. A rename storm must not scan the entire
    // baseline for each newly named file. BTreeMap order keeps tie-breaking stable.
    let mut removed: BTreeMap<String, Vec<(&String, &File)>> = BTreeMap::new();
    for (path, file) in base.iter().filter(|(path, _)| !head.contains_key(*path)) {
        check_deadline(deadline)?;
        removed
            .entry(super::digest(&file.bytes))
            .or_default()
            .push((path, file));
    }
    let mut renamed = BTreeSet::new();
    for (path, file) in head {
        check_deadline(deadline)?;
        let old = base.get(path);
        if old.is_some_and(|old| old.bytes == file.bytes && old.executable == file.executable) {
            continue;
        }
        let moved = if old.is_none() {
            removed
                .get(&super::digest(&file.bytes))
                .and_then(|entries| {
                    entries
                        .iter()
                        .copied()
                        .find(|(_, old)| old.bytes == file.bytes)
                })
        } else {
            None
        };
        if let Some((path, _)) = moved {
            renamed.insert(path);
        }
        let old_bytes = old.map(|file| file.bytes.as_slice()).unwrap_or_default();
        let old_text = String::from_utf8_lossy(old_bytes);
        let text = String::from_utf8_lossy(&file.bytes);
        let mut removed_lines = BTreeSet::new();
        let added_lines = if moved.is_some() || file.bytes.contains(&0) || old_bytes.contains(&0) {
            BTreeSet::new()
        } else {
            let mut config = TextDiff::configure();
            if let Some(deadline) = deadline {
                config.deadline(deadline);
            }
            let diff = config.diff_lines(old_text.as_ref(), text.as_ref());
            // The diff library may return an approximation when its deadline
            // expires. Such partial mapping cannot become successful evidence.
            check_deadline(deadline)?;
            removed_lines.extend(
                diff.iter_all_changes()
                    .filter(|change| change.tag() == ChangeTag::Delete)
                    .filter_map(|change| change.old_index().map(|index| index + 1)),
            );
            diff.iter_all_changes()
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
                removed_lines,
            },
        );
    }
    for path in base.keys().filter(|path| !head.contains_key(*path)) {
        check_deadline(deadline)?;
        if renamed.contains(path) {
            continue;
        }
        changes.insert(
            path.clone(),
            Change {
                kind: "deleted".into(),
                old_path: Some(path.clone()),
                removed_lines: Default::default(),
                added_lines: BTreeSet::new(),
            },
        );
    }
    check_deadline(deadline)?;
    Ok(changes)
}
