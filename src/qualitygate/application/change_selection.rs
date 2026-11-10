//! Derive scheduling evidence from both sides of the complete immutable change.
use crate::{
    config,
    domain::change_selection::{ChangedInput, Selection},
    snapshot::Snapshot,
};
use anyhow::Result;
use std::{collections::BTreeMap, sync::Arc};

pub(super) async fn select(
    plan: &config::Plan,
    snapshot: &Arc<Snapshot>,
    protected_paths: &[String],
    policy_invalid: bool,
) -> Result<BTreeMap<String, Selection>> {
    if plan
        .commands
        .iter()
        .all(|check| check.when_changed.is_none())
    {
        return Ok(plan
            .commands
            .iter()
            .map(|check| {
                (
                    check.id.clone(),
                    Selection::new(
                        true,
                        "No when_changed condition; execution is unconditional",
                    ),
                )
            })
            .collect());
    }
    let (plan, snapshot, protected_paths) =
        (plan.clone(), Arc::clone(snapshot), protected_paths.to_vec());
    tokio::task::spawn_blocking(move || {
        let mut builder = globset::GlobSetBuilder::new();
        for pattern in protected_paths {
            builder.add(globset::Glob::new(&pattern)?);
        }
        let protected = builder.build()?;
        let mut changes = Vec::new();
        let mut shared = None;
        for (path, change) in &snapshot.changes {
            for (path, file) in std::iter::once((path, snapshot.files.get(path))).chain(
                change
                    .old_path
                    .as_ref()
                    .map(|path| (path, snapshot.base_files.get(path))),
            ) {
                if shared.is_none()
                    && (protected.is_match(path)
                        || config::change_selection::shared_build_input(path))
                {
                    shared = Some(format!(
                        "Policy, verification or shared build input changed: {path}"
                    ));
                }
                changes.push(ChangedInput {
                    path: path.clone(),
                    language: crate::domain::language::for_text_source(
                        path,
                        file.map_or(&[], |file| file.bytes.as_slice()),
                    )
                    .map(str::to_owned),
                });
                // A .h header can affect either C or C++ translation units.
                // Text rules keep their conventional C classification; command
                // scheduling must conservatively retain both consumers.
                if path.ends_with(".h") {
                    changes.push(ChangedInput {
                        path: path.clone(),
                        language: Some("cpp".into()),
                    });
                }
            }
        }
        let force = if policy_invalid {
            Some("Policy validation did not complete; non-applicability cannot be established")
        } else if !snapshot.delivery() {
            Some("Repository verification requires all selected commands")
        } else {
            shared.as_deref()
        };
        config::change_selection::select(&plan, &changes, force)
    })
    .await?
}

#[cfg(test)]
#[path = "change_selection_tests.rs"]
mod tests;
