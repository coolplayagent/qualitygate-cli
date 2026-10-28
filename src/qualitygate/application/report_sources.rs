//! Resolve report paths against immutable inputs without repeatedly scanning them.

use crate::{paths, snapshot::Snapshot};
use anyhow::{Result, bail};
use std::{cmp::Ordering, path::Path};

pub(super) struct Sources<'a> {
    snapshot: &'a Snapshot,
    workspace: &'a Path,
    current: Option<Vec<&'a str>>,
    baseline: Option<Vec<&'a str>>,
}

impl<'a> Sources<'a> {
    pub(super) fn new(snapshot: &'a Snapshot, workspace: &'a Path) -> Self {
        Self {
            snapshot,
            workspace,
            current: None,
            baseline: None,
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
}

fn reversed(left: &str, right: &str) -> Ordering {
    left.bytes().rev().cmp(right.bytes().rev())
}

#[cfg(test)]
#[path = "report_sources_tests.rs"]
mod tests;
