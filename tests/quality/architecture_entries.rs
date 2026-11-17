//! Cargo's production targets must exactly match the reviewed architecture roots.

use std::{collections::BTreeSet, path::Path, time::Duration};

type Entry = (String, String, String);

fn inventory(metadata: &serde_json::Value, root: &Path) -> Result<Vec<Entry>, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Missing package inventory")?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("Missing workspace inventory")?;
    let mut entries = BTreeSet::new();
    for package in packages
        .iter()
        .filter(|package| members.contains(&package["id"]))
    {
        for target in package["targets"]
            .as_array()
            .ok_or("Missing target inventory")?
        {
            let kinds = target["kind"].as_array().ok_or("Missing target kind")?;
            let Some(kind) = kinds.iter().filter_map(|kind| kind.as_str()).find(|kind| {
                matches!(
                    *kind,
                    "lib"
                        | "bin"
                        | "rlib"
                        | "dylib"
                        | "cdylib"
                        | "staticlib"
                        | "proc-macro"
                        | "custom-build"
                )
            }) else {
                continue;
            };
            let name = target["name"].as_str().ok_or("Missing target name")?;
            let source = target["src_path"].as_str().ok_or("Missing target source")?;
            let relative = Path::new(source)
                .strip_prefix(root)
                .map_err(|_| "Production source is outside the repository")?;
            if relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("Production source must have a normalized relative path".into());
            }
            if !entries.insert((
                name.into(),
                kind.into(),
                relative.to_string_lossy().replace('\\', "/"),
            )) {
                return Err("Duplicate production target".into());
            }
        }
    }
    let expected: BTreeSet<_> = super::owner_contract::PRODUCTION_ENTRIES
        .iter()
        .map(|(name, kind, path)| (name.to_string(), kind.to_string(), path.to_string()))
        .collect();
    if entries != expected {
        return Err(format!(
            "Production targets differ from reviewed architecture roots: actual={entries:?}; expected={expected:?}"
        ));
    }
    Ok(entries.into_iter().collect())
}

pub(super) async fn production_entries(root: &Path) -> Vec<Entry> {
    let output = qualitygate::runner::capture(
        &[
            env!("CARGO").into(),
            "metadata".into(),
            "--no-deps".into(),
            "--locked".into(),
            "--offline".into(),
            "--format-version".into(),
            "1".into(),
        ],
        root,
        None,
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert!(
        !output.timed_out && output.capture_error.is_none(),
        "Cargo target inventory was incomplete"
    );
    assert_eq!(
        output.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    inventory(&metadata, root).unwrap()
}

#[test]
fn architecture_rejects_unregistered_missing_and_outside_production_entries() {
    let root = Path::new("/repository");
    let targets: Vec<_> = super::owner_contract::PRODUCTION_ENTRIES.iter().map(|(name, kind, path)| {
        serde_json::json!({"name":name,"kind":[kind],"src_path":root.join(path)})
    }).collect();
    let original = serde_json::json!({"workspace_members":["package"],"packages":[{"id":"package","targets":targets}]});
    assert_eq!(inventory(&original, root).unwrap().len(), 3);
    let mut unregistered = original.clone();
    unregistered["packages"][0]["targets"].as_array_mut().unwrap().push(serde_json::json!({"name":"hidden","kind":["bin"],"src_path":"/repository/src/hidden.rs"}));
    assert!(
        inventory(&unregistered, root)
            .unwrap_err()
            .contains("reviewed architecture roots")
    );
    let mut missing = original.clone();
    missing["packages"][0]["targets"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(inventory(&missing, root).is_err());
    let mut outside = original;
    outside["packages"][0]["targets"][0]["src_path"] = "/outside/main.rs".into();
    assert!(
        inventory(&outside, root)
            .unwrap_err()
            .contains("outside the repository")
    );
}
