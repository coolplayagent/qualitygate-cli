use super::*;
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct Plan {
    pub rules: BTreeMap<String, RuleSetting>,
    pub commands: Vec<CommandCheck>,
    pub required: Vec<String>,
    pub pending_delivery: Vec<String>,
    pub task_id: Option<String>,
    pub acceptance: BTreeMap<String, String>,
}

pub fn parse_task(bytes: &[u8]) -> Result<TaskContract> {
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Task contract exceeds configuration budget");
    }
    let task: TaskContract = serde_norway::from_slice(bytes)?;
    if task.schema_version != 1 || task.task_id.trim().is_empty() || task.acceptance.is_empty() {
        bail!("Task contract needs schema_version 1, task_id and acceptance items");
    }
    let mut ids = BTreeSet::new();
    for item in &task.acceptance {
        validation::validate_id(&item.id)?;
        if !ids.insert(&item.id) || item.description.trim().is_empty() {
            bail!("Task acceptance IDs must be unique and descriptions nonempty");
        }
    }
    Ok(task)
}

impl Plan {
    pub fn build(config: &Config, task: Option<&TaskContract>, profile: &str) -> Result<Self> {
        if !["quick", "full"].contains(&profile) {
            bail!("Unknown profile: {profile}");
        }
        let mut combined = config.clone();
        let mut acceptance = BTreeMap::new();
        if let Some(task) = task {
            for item in &task.acceptance {
                let v = &item.verification;
                combined.checks.push(CommandCheck {
                    id: v.check_id.clone(),
                    kind: v.kind,
                    argv: v.argv.clone(),
                    cwd: v.cwd.clone(),
                    timeout_seconds: v.timeout_seconds,
                    required: item.required,
                    severity: item.severity,
                    depends_on: Vec::new(),
                    reports: v.reports.clone(),
                    expected_exit_code: v.expected_exit_code,
                    findings_exit_codes: v.findings_exit_codes.clone(),
                    tools: v.tools.clone(),
                    required_args: Vec::new(),
                    evidence_file: v.evidence_file.clone(),
                });
                if let Some(full) = combined.profiles.get_mut("full") {
                    full.include.push(v.check_id.clone());
                }
                acceptance.insert(item.id.clone(), v.check_id.clone());
            }
        }
        validation::validate(&combined)?;
        let all_required: BTreeSet<String> = combined
            .rules
            .iter()
            .filter(|(_, rule)| rule.enabled && rule.required)
            .map(|(id, _)| id.clone())
            .chain(
                combined
                    .checks
                    .iter()
                    .filter(|check| check.required)
                    .map(|check| check.id.clone()),
            )
            .collect();
        let selected: BTreeSet<String> = if let Some(profile) = combined.profiles.get(profile) {
            profile.include.iter().cloned().collect()
        } else if profile == "quick" {
            combined.rules.keys().cloned().collect()
        } else {
            combined
                .rules
                .keys()
                .cloned()
                .chain(combined.checks.iter().map(|check| check.id.clone()))
                .collect()
        };
        let mut needed = selected;
        for _ in 0..combined.checks.len() {
            for check in &combined.checks {
                if needed.contains(&check.id) {
                    needed.extend(check.depends_on.iter().cloned());
                }
            }
        }
        let rules: BTreeMap<_, _> = combined
            .rules
            .into_iter()
            .filter(|(id, rule)| rule.enabled && needed.contains(id))
            .collect();
        let mut remaining: Vec<_> = combined
            .checks
            .into_iter()
            .filter(|check| needed.contains(&check.id))
            .collect();
        let mut commands = Vec::new();
        let mut ordered: BTreeSet<_> = rules.keys().cloned().collect();
        while !remaining.is_empty() {
            let Some(index) = remaining
                .iter()
                .position(|check| check.depends_on.iter().all(|id| ordered.contains(id)))
            else {
                bail!("Check depends on a disabled or unavailable prerequisite");
            };
            let check = remaining.remove(index);
            ordered.insert(check.id.clone());
            commands.push(check);
        }
        let required = all_required.intersection(&ordered).cloned().collect();
        let pending_delivery = all_required.difference(&ordered).cloned().collect();
        Ok(Self {
            rules,
            commands,
            required,
            pending_delivery,
            task_id: task.map(|task| task.task_id.clone()),
            acceptance,
        })
    }
}
