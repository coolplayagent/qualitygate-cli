//! Borrowed lookup index for repeated delivery-scope decisions.
use super::{Change, Snapshot};
use crate::domain::Range;
use std::collections::BTreeMap;

/// An operation-local index, never cached on the publicly mutable snapshot.
pub struct ScopeIndex<'a> {
    snapshot: &'a Snapshot,
    old_paths: BTreeMap<&'a str, &'a Change>,
}

impl<'a> ScopeIndex<'a> {
    pub(super) fn new(snapshot: &'a Snapshot) -> Self {
        let mut old_paths = BTreeMap::new();
        if snapshot.delivery() {
            for change in snapshot.changes.values() {
                if let Some(path) = change.old_path.as_deref() {
                    // Match the first value in the snapshot's ordered map, even
                    // for caller-constructed changes with duplicate old paths.
                    old_paths.entry(path).or_insert(change);
                }
            }
        }
        Self {
            snapshot,
            old_paths,
        }
    }

    pub fn includes(&self, file: &str) -> bool {
        (!self.snapshot.delivery()
            || self.snapshot.changes.contains_key(file)
            || self.old_paths.contains_key(file))
            && self.snapshot.feedback_includes(file)
    }

    pub fn selects_diagnostic(&self, file: Option<&str>, range: Option<&Range>) -> bool {
        let Some(file) = file else { return true };
        if !self.includes(file) {
            return false;
        }
        if !self.snapshot.delivery() {
            return true;
        }
        // Current keys take precedence over old aliases, as in the single-call API.
        let change = self
            .snapshot
            .changes
            .get(file)
            .or_else(|| self.old_paths.get(file).copied());
        change.is_some_and(|change| {
            range.is_none_or(|range| {
                change
                    .added_lines
                    .range(range.start_line..=range.end_line)
                    .next()
                    .is_some()
            })
        })
    }
}

#[cfg(test)]
#[path = "scope_index_tests.rs"]
mod tests;
