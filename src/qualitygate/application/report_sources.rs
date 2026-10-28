//! Resolve report paths against immutable inputs without repeatedly scanning them.

use crate::{paths, snapshot::Snapshot};
use anyhow::{Result, bail};
use std::{cmp::Ordering, collections::BTreeMap, path::Path};

pub(super) struct Sources<'a> {
    snapshot: &'a Snapshot,
    workspace: &'a Path,
    current: Option<Vec<&'a str>>,
    baseline: Option<Vec<&'a str>>,
    renames: Option<BTreeMap<&'a str, &'a str>>,
}

impl<'a> Sources<'a> {
    pub(super) fn new(snapshot: &'a Snapshot, workspace: &'a Path) -> Self {
        Self {
            snapshot,
            workspace,
            current: None,
            baseline: None,
            renames: None,
        }
    }

    pub(super) fn map(&mut self, file: &str, base: bool) -> Result<String> {
        let files = if base {
            &self.snapshot.base_files
        } else {
            &self.snapshot.files
        };
        let path = Path::new(file.strip_prefix("file://").unwrap_or(file));
        let path = path
            .strip_prefix(self.workspace)
            .or_else(|_| path.strip_prefix(&self.snapshot.root))
            .unwrap_or(path);
        let relative = paths::from_native(path)?;
        if files.contains_key(&relative) {
            return Ok(relative);
        }
        let index = if base {
            &mut self.baseline
        } else {
            &mut self.current
        };
        let names = index.get_or_insert_with(|| {
            let mut names: Vec<_> = files.keys().map(String::as_str).collect();
            names.sort_unstable_by(|left, right| reversed(left, right));
            names
        });
        // Reversed byte ordering groups suffixes contiguously. The leading slash
        // requires a component boundary; a second match proves ambiguity.
        let suffix = format!("/{relative}");
        let first = names.partition_point(|name| reversed(name, &suffix).is_lt());
        if let Some(name) = names.get(first)
            && name.ends_with(&suffix)
            && names
                .get(first + 1)
                .is_none_or(|next| !next.ends_with(&suffix))
        {
            return Ok((*name).to_owned());
        }
        bail!("Report path cannot be mapped uniquely to checked sources: {file}")
    }

    /// Call after validating the location against its original baseline source.
    pub(super) fn remap_baseline(&mut self, file: String) -> String {
        let renames = self.renames.get_or_insert_with(|| {
            let mut renames = BTreeMap::new();
            for (path, change) in &self.snapshot.changes {
                if change.kind == "renamed"
                    && let Some(old) = change.old_path.as_deref()
                {
                    // Preserve the first target in snapshot order if old paths repeat.
                    renames.entry(old).or_insert(path.as_str());
                }
            }
            renames
        });
        renames
            .get(file.as_str())
            .map(|path| (*path).to_owned())
            .unwrap_or(file)
    }
}

fn reversed(left: &str, right: &str) -> Ordering {
    left.bytes().rev().cmp(right.bytes().rev())
}

#[cfg(test)]
#[path = "report_sources_tests.rs"]
mod tests;
