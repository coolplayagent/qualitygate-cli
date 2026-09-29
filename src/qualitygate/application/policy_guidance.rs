//! Reliable follow-up actions for missing policy inputs.

use crate::{
    config,
    domain::{NextStep, PolicyInputError},
    paths,
};
use anyhow::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum SelectionKind {
    Worktree,
    Staged,
    Diff,
    MergeRequest,
    Path,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub root: PathBuf,
    pub config: String,
    pub selection: Option<SelectionKind>,
    pub policy_ref: bool,
}

pub async fn next_steps(error: &Error, context: Context) -> Result<Vec<NextStep>> {
    if let Some(error) = error.downcast_ref::<crate::domain::runtime::ConfigurationError>() {
        let incompatible = error.code.starts_with("runtime.");
        return Ok(vec![NextStep {
            action: if incompatible { "select_compatible_runtime" } else { "repair_configuration" }.into(),
            command: incompatible.then(|| vec!["qualitygate".into(), "capabilities".into(), "--format".into(), "json".into()]),
            message: if incompatible {
                "Select a verified CLI/Skill build satisfying the declared version and capability IDs, inspect its capabilities, then retry. No tool is installed automatically."
            } else {
                "Repair the reported YAML syntax, type or field using its original error and location. An unknown field alone does not establish that the runtime is outdated."
            }.into(),
        }]);
    }
    let Some(failure) = error
        .chain()
        .find_map(|source| source.downcast_ref::<PolicyInputError>())
        .cloned()
    else {
        return Ok(Vec::new());
    };
    Ok(tokio::task::spawn_blocking(move || plan(&failure, &context)).await?)
}

fn plan(failure: &PolicyInputError, context: &Context) -> Vec<NextStep> {
    let Ok(root) = dunce::canonicalize(&context.root) else {
        return Vec::new();
    };
    match failure {
        PolicyInputError::LocalConfigurationMissing { .. } => {
            initialize(&root, &context.config).into_iter().collect()
        }
        PolicyInputError::SnapshotConfigurationMissing { .. } => {
            if context.policy_ref {
                return vec![NextStep {
                    action: "select_policy_reference".into(),
                    command: None,
                    message: "Select a policy reference containing the reviewed configuration, or commit it and use the new reference.".into(),
                }];
            }
            let Some(selection) = context.selection else {
                return Vec::new();
            };
            let Some(local_exists) = local_config_exists(&root, &context.config) else {
                return Vec::new();
            };
            let mut steps = Vec::new();
            if !local_exists {
                let Some(step) = initialize(&root, &context.config) else {
                    return Vec::new();
                };
                steps.push(step);
            }
            let message = match selection {
                SelectionKind::Worktree if !local_exists => None,
                SelectionKind::Worktree | SelectionKind::Path => Some((
                    "include_policy_in_snapshot",
                    "Ensure the reviewed configuration is included in the selected worktree snapshot, then retry.",
                )),
                SelectionKind::Staged => Some((
                    "include_policy_in_snapshot",
                    "Stage the reviewed configuration, then retry the staged check.",
                )),
                SelectionKind::Diff | SelectionKind::MergeRequest => Some((
                    "include_policy_in_snapshot",
                    "Commit the reviewed configuration in the selected head, then retry the check.",
                )),
            };
            if let Some((action, message)) = message {
                steps.push(NextStep {
                    action: action.into(),
                    command: None,
                    message: message.into(),
                });
            }
            steps
        }
    }
}

fn local_config_exists(root: &Path, config: &str) -> Option<bool> {
    let file = paths::confined(root, Path::new(config)).ok()?;
    match std::fs::metadata(file) {
        Ok(metadata) if metadata.is_file() => Some(true),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}

fn initialize(root: &Path, config_path: &str) -> Option<NextStep> {
    let root = dunce::simplified(root).to_str()?;
    let config_path = paths::relative(Path::new(config_path)).ok()?;
    let mut command = vec!["qualitygate".into(), "--root".into(), root.into()];
    if config_path != config::CONFIG_FILE {
        command.extend(["--config".into(), config_path]);
    }
    command.extend(["init".into(), "--format".into(), "json".into()]);
    Some(NextStep {
        action: "initialize_candidate_policy".into(),
        command: Some(command),
        message: "Review the generated candidate policy, then retry.".into(),
    })
}
