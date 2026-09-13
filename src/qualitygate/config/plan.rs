use super::*;
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct Plan {
    pub rules: BTreeMap<String, RuleSetting>,
    pub commands: Vec<CommandCheck>,
    pub order: Vec<String>,
    pub required: Vec<String>,
    pub pending_delivery: Vec<String>,
    pub task_id: Option<String>,
    pub acceptance: BTreeMap<String, String>,
    pub acceptance_descriptions: BTreeMap<String, String>,
}

pub fn parse_task(bytes: &[u8]) -> Result<TaskContract> {
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Task contract exceeds configuration budget");
    }
    let task: TaskContract = super::parse_yaml(bytes)?;
    validate_task(&task)?;
    Ok(task)
}

fn validate_task(task: &TaskContract) -> Result<()> {
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
    Ok(())
}

impl Plan {
    pub fn build(config: &Config, task: Option<&TaskContract>, profile: &str) -> Result<Self> {
        if !["quick", "full"].contains(&profile) {
            bail!("Unknown profile: {profile}");
        }
        let mut combined = config.clone();
        let mut acceptance = BTreeMap::new();
        let mut acceptance_descriptions = BTreeMap::new();
        if let Some(task) = task {
            validate_task(task)?;
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
                    depends_on: v.depends_on.clone(),
                    reports: v.reports.clone(),
                    projects: v.projects.clone(),
                    expected_exit_code: v.expected_exit_code,
                    findings_exit_codes: v.findings_exit_codes.clone(),
                    tools: v.tools.clone(),
                    required_args: v.required_args.clone(),
                    evidence_file: v.evidence_file.clone(),
                    compatibility: v.compatibility.clone(),
                });
                if let Some(full) = combined.profiles.get_mut("full") {
                    full.include.push(v.check_id.clone());
                }
                acceptance.insert(item.id.clone(), v.check_id.clone());
                acceptance_descriptions.insert(item.id.clone(), item.description.clone());
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
        for _ in 0..combined.checks.len() + combined.rules.len() {
            for (id, rule) in &combined.rules {
                if needed.contains(id) {
                    needed.extend(rule.depends_on.iter().cloned());
                }
            }
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
        let commands: Vec<_> = combined
            .checks
            .into_iter()
            .filter(|check| needed.contains(&check.id))
            .collect();
        let mut remaining: Vec<_> = rules
            .iter()
            .map(|(id, rule)| (id.clone(), rule.depends_on.clone()))
            .chain(
                commands
                    .iter()
                    .map(|check| (check.id.clone(), check.depends_on.clone())),
            )
            .collect();
        let mut ordered = BTreeSet::new();
        let mut order = Vec::new();
        while !remaining.is_empty() {
            let Some(index) = remaining
                .iter()
                .position(|(_, dependencies)| dependencies.iter().all(|id| ordered.contains(id)))
            else {
                bail!("Check depends on a disabled or unavailable prerequisite");
            };
            let (id, _) = remaining.remove(index);
            ordered.insert(id.clone());
            order.push(id);
        }
        let required = all_required.intersection(&ordered).cloned().collect();
        let pending_delivery = all_required.difference(&ordered).cloned().collect();
        Ok(Self {
            rules,
            commands,
            order,
            required,
            pending_delivery,
            task_id: task.map(|task| task.task_id.clone()),
            acceptance,
            acceptance_descriptions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reusable_planning_validates_task_contracts_without_requiring_yaml_loading() {
        let config = super::super::parse(b"schema_version: 1\nrules: {line-ending: {}}\n").unwrap();
        let valid = json!({"schema_version":1,"task_id":"behavior","acceptance":[{
            "id":"condition","description":"Observable condition","verification":{"check_id":"verify","argv":["git","--version"]}
        }]});
        let task: TaskContract = serde_json::from_value(valid.clone()).unwrap();
        assert!(Plan::build(&config, Some(&task), "full").is_ok());
        for field in [
            "schema",
            "task_id",
            "empty",
            "id",
            "description",
            "duplicate",
        ] {
            let mut value = valid.clone();
            match field {
                "schema" => value["schema_version"] = json!(2),
                "task_id" => value["task_id"] = json!(" "),
                "empty" => value["acceptance"] = json!([]),
                "id" => value["acceptance"][0]["id"] = json!("invalid id"),
                "description" => value["acceptance"][0]["description"] = json!(""),
                _ => {
                    let mut other = value["acceptance"][0].clone();
                    other["verification"]["check_id"] = json!("another-check");
                    value["acceptance"].as_array_mut().unwrap().push(other);
                }
            }
            let task = serde_json::from_value(value).unwrap();
            for profile in ["quick", "full"] {
                assert!(
                    Plan::build(&config, Some(&task), profile).is_err(),
                    "{field}, {profile}"
                );
            }
        }
    }
}
