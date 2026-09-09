//! Maven effective-model and dependency-plugin JSON normalization; no import guessing.

use crate::{
    config::MavenProject,
    domain::{DependencyFact, ProjectFacts},
    paths,
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
use roxmltree::Node;
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

#[cfg(test)]
#[path = "maven_tests.rs"]
mod tests;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tree {
    group_id: String,
    artifact_id: String,
    version: String,
    #[serde(rename = "type")]
    artifact_type: String,
    classifier: String,
    scope: String,
    optional: String,
    #[serde(default)]
    children: Vec<Tree>,
}

pub fn parse(
    spec: &MavenProject,
    model: &[u8],
    tree: &[u8],
    workspace: &Path,
    snapshot: &Snapshot,
    producer: &str,
) -> Result<ProjectFacts> {
    if model.len() > crate::snapshot::MAX_FILE_BYTES || tree.len() > crate::snapshot::MAX_FILE_BYTES
    {
        bail!("Maven output exceeds report size budget");
    }
    let manifest = if spec.root == "." {
        "pom.xml".into()
    } else {
        format!("{}/pom.xml", spec.root)
    };
    let input = snapshot
        .files
        .get(&manifest)
        .context("Maven project manifest is absent from snapshot")?;
    let input = roxmltree::Document::parse(std::str::from_utf8(&input.bytes)?)?;
    let model = roxmltree::Document::parse(std::str::from_utf8(model)?)?;
    let project = model.root_element();
    if !project.has_tag_name("project") || text(project, "modelVersion")? != "4.0.0" {
        bail!("Expected one Maven 4.0.0 effective project, not a reactor or raw report");
    }
    let group = text(project, "groupId")?;
    let artifact = text(project, "artifactId")?;
    let version = text(project, "version")?;
    if text(input.root_element(), "artifactId")? != artifact {
        bail!("Effective Maven artifact does not match snapshot manifest");
    }
    let build = child(project, "build")?.context("Effective POM has no build roots")?;
    let source_root = source_path(text(build, "sourceDirectory")?, workspace, &spec.root)?;
    let test_source_root = source_path(text(build, "testSourceDirectory")?, workspace, &spec.root)?;
    if source_root == test_source_root {
        bail!("Maven main and test source roots are ambiguous");
    }
    let mut declared = Vec::new();
    let mut keys = BTreeSet::new();
    if let Some(dependencies) = child(project, "dependencies")? {
        for dependency in dependencies.children().filter(|node| node.is_element()) {
            if !dependency.has_tag_name("dependency") {
                bail!("Unexpected effective dependency element");
            }
            let fact = DependencyFact {
                group: text(dependency, "groupId")?.into(),
                artifact: text(dependency, "artifactId")?.into(),
                version: text(dependency, "version")?.into(),
                artifact_type: optional(dependency, "type", "jar")?,
                classifier: optional(dependency, "classifier", "")?,
                scope: optional(dependency, "scope", "compile")?,
            };
            validate(&fact)?;
            if !keys.insert((
                fact.group.clone(),
                fact.artifact.clone(),
                fact.artifact_type.clone(),
                fact.classifier.clone(),
            )) {
                bail!("Ambiguous duplicate effective Maven dependency");
            }
            declared.push(fact);
        }
    }
    let tree: Tree = serde_json::from_slice(tree)?;
    if tree.group_id != group
        || tree.artifact_id != artifact
        || tree.version != version
        || !tree.scope.is_empty()
    {
        bail!("Maven dependency tree and effective model identify different projects");
    }
    let mut resolved = Vec::new();
    let mut direct = BTreeSet::new();
    let mut pending = vec![(&tree, 0)];
    while let Some((node, depth)) = pending.pop() {
        if depth > 64 || resolved.len() > 50_000 {
            bail!("Maven dependency tree exceeds analysis budget");
        }
        let fact = DependencyFact {
            group: node.group_id.clone(),
            artifact: node.artifact_id.clone(),
            version: node.version.clone(),
            artifact_type: node.artifact_type.clone(),
            classifier: node.classifier.clone(),
            scope: node.scope.clone(),
        };
        if !["true", "false"].contains(&node.optional.as_str()) {
            bail!("Invalid Maven optional flag");
        }
        if depth > 0 {
            validate(&fact)?;
            if depth == 1 {
                direct.insert(fact.clone());
            }
            resolved.push(fact);
        }
        pending.extend(node.children.iter().map(|child| (child, depth + 1)));
    }
    // Missing declared nodes mean a filtered, truncated or incomparable resolution.
    // Such evidence cannot establish dependency absence.
    if direct != declared.iter().cloned().collect() {
        bail!("Maven dependency tree omits or contradicts an effective declared dependency");
    }
    Ok(ProjectFacts {
        schema_version: 1,
        ecosystem: "maven".into(),
        root: spec.root.clone(),
        manifest,
        coordinate: format!("{group}:{artifact}:{version}"),
        source_root,
        test_source_root,
        declared,
        resolved,
        dependency_usage: None,
        python: None,
        producer_check: producer.into(),
        snapshot_digest: snapshot.identity.content_digest.clone(),
    })
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Result<Option<Node<'a, 'input>>> {
    let mut nodes = node.children().filter(|node| node.has_tag_name(name));
    let first = nodes.next();
    if nodes.next().is_some() {
        bail!("Duplicate Maven model element: {name}");
    }
    Ok(first)
}

fn text<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Result<&'a str> {
    let value = child(node, name)?
        .and_then(|node| node.text())
        .context(format!("Missing Maven model {name}"))?
        .trim();
    if value.is_empty() || value.contains("${") {
        bail!("Unresolved Maven model {name}");
    }
    Ok(value)
}

fn optional(node: Node<'_, '_>, name: &str, default: &str) -> Result<String> {
    if child(node, name)?.is_some() {
        Ok(text(node, name)?.into())
    } else {
        Ok(default.into())
    }
}

fn validate(fact: &DependencyFact) -> Result<()> {
    for value in [
        &fact.group,
        &fact.artifact,
        &fact.version,
        &fact.artifact_type,
    ] {
        if value.is_empty() || value.contains("${") || value.chars().any(char::is_whitespace) {
            bail!("Unresolved or empty Maven dependency coordinate");
        }
    }
    if !["compile", "provided", "runtime", "test", "system"].contains(&fact.scope.as_str()) {
        bail!("Unsupported Maven dependency scope: {}", fact.scope);
    }
    Ok(())
}

fn source_path(value: &str, workspace: &Path, root: &str) -> Result<String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        bail!("Effective Maven source roots must be absolute");
    }
    let path = paths::from_native(
        path.strip_prefix(workspace)
            .context("Maven source root escapes checked workspace")?,
    )?;
    if path.is_empty()
        || (root != "."
            && !path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/')))
    {
        bail!("Maven source root escapes its configured module");
    }
    Ok(path)
}
