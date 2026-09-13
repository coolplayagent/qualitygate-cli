//! Validate analyzer identity, complete archive inventories and japicmp XML.

use crate::{config::compatibility::CompatibilityLevel, snapshot};
use anyhow::{Context, Result, bail};
use roxmltree::Node;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

pub const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;

#[derive(Debug, Serialize)]
pub struct Finding {
    pub symbol: String,
    pub change: String,
    pub binary_compatible: bool,
    pub source_compatible: bool,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub classes_compared: usize,
    pub binary_compatible: bool,
    pub source_compatible: bool,
    pub findings: Vec<Finding>,
}

fn archive(bytes: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        bail!("JAR exceeds 32 MiB");
    }
    let archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > MAX_ENTRIES {
        bail!("JAR exceeds 50000 entries");
    }
    Ok(archive)
}

pub fn analyzer_version(bytes: &[u8], expected_digest: &str) -> Result<String> {
    if snapshot::digest(bytes) != expected_digest {
        bail!("Compatibility analyzer digest does not match trusted policy");
    }
    let mut archive = archive(bytes)?;
    let mut file = archive
        .by_name("META-INF/maven/com.github.siom79.japicmp/japicmp/pom.properties")
        .context("Analyzer lacks japicmp version metadata")?;
    if file.size() > 16_384 {
        bail!("Analyzer version metadata exceeds its budget");
    }
    let mut text = String::new();
    file.by_ref().take(16_385).read_to_string(&mut text)?;
    if text.len() > 16_384 {
        bail!("Analyzer version metadata exceeds its budget");
    }
    let mut fields = BTreeMap::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (key, value) = line
            .split_once('=')
            .context("Unsupported analyzer version metadata")?;
        if fields.insert(key.trim(), value.trim()).is_some() {
            bail!("Duplicate analyzer version field");
        }
    }
    if fields.get("groupId") != Some(&"com.github.siom79.japicmp")
        || fields.get("artifactId") != Some(&"japicmp")
        || fields.get("version") != Some(&"0.26.2")
    {
        bail!("Compatibility adapter requires japicmp 0.26.2 metadata");
    }
    Ok("0.26.2".into())
}

pub fn classes(bytes: &[u8]) -> Result<BTreeSet<String>> {
    let mut archive = archive(bytes)?;
    let mut names = BTreeSet::new();
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        expanded = expanded
            .checked_add(file.size())
            .context("JAR size overflow")?;
        if expanded > 256 * 1024 * 1024 {
            bail!("Expanded JAR exceeds 256 MiB");
        }
        let name = file.name();
        if file.enclosed_name().is_none() || name.contains('\\') {
            bail!("JAR contains an unsafe entry");
        }
        if name.starts_with("META-INF/versions/") {
            bail!("Multi-release JARs require an explicit runtime-version adapter");
        }
        if let Some(class) = name.strip_suffix(".class") {
            if class == "module-info" {
                bail!("JPMS module descriptors require a module-compatibility adapter");
            }
            if class.starts_with("META-INF/")
                || class.is_empty()
                || !names.insert(class.replace('/', "."))
            {
                bail!("JAR class inventory is ambiguous");
            }
        }
    }
    Ok(names)
}

fn attribute<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str> {
    node.attribute(name)
        .with_context(|| format!("Missing japicmp attribute {name}"))
}

fn boolean(node: Node<'_, '_>, name: &str) -> Result<bool> {
    match attribute(node, name)? {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => bail!("Invalid japicmp boolean {name}"),
    }
}

fn incompatible(binary: bool, source: bool, level: CompatibilityLevel) -> bool {
    match level {
        CompatibilityLevel::Binary => !binary,
        CompatibilityLevel::Source => !source,
        CompatibilityLevel::Both => !binary || !source,
    }
}

fn symbol(node: Node<'_, '_>, class: &str) -> Result<String> {
    if let Some(member) = node
        .ancestors()
        .find(|node| ["method", "constructor", "field"].contains(&node.tag_name().name()))
    {
        let name = attribute(member, "name")?;
        if member.has_tag_name("field") {
            return Ok(format!("{class}#{name}"));
        }
        let parameters = member
            .children()
            .find(|node| node.has_tag_name("parameters"))
            .context("Missing japicmp method parameter inventory")?;
        let types: Result<Vec<_>> = parameters
            .children()
            .filter(Node::is_element)
            .map(|parameter| attribute(parameter, "type"))
            .collect();
        Ok(format!("{class}#{name}({})", types?.join(",")))
    } else {
        Ok(class.into())
    }
}

pub fn parse(
    bytes: &[u8],
    old: &str,
    new: &str,
    expected_classes: &BTreeSet<String>,
    level: CompatibilityLevel,
    inventory: bool,
) -> Result<Comparison> {
    if bytes.len() > snapshot::MAX_FILE_BYTES {
        bail!("Compatibility report exceeds its byte budget");
    }
    let document = roxmltree::Document::parse(std::str::from_utf8(bytes)?)?;
    let root = document.root_element();
    if !root.has_tag_name("japicmp")
        || attribute(root, "oldJar")? != old
        || attribute(root, "newJar")? != new
        || attribute(root, "accessModifier")? != if inventory { "PRIVATE" } else { "PROTECTED" }
        || attribute(root, "packagesInclude")? != "all"
        || attribute(root, "packagesExclude")? != "n.a."
        || boolean(root, "ignoreMissingClasses")?
        || !attribute(root, "ignoreMissingClassesByRegularExpressions")?.is_empty()
        || boolean(root, "onlyModifications")?
        || boolean(root, "onlyBinaryIncompatibleModifications")?
    {
        bail!("Compatibility report is filtered, incomplete or bound to different archives");
    }
    let inventories: Vec<_> = root
        .children()
        .filter(|node| node.has_tag_name("classes"))
        .collect();
    if inventories.len() != 1 {
        bail!("Compatibility report needs one complete class inventory");
    }
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    let (mut all_binary, mut all_source) = (true, true);
    for class in inventories[0].children().filter(Node::is_element) {
        if !class.has_tag_name("class") {
            bail!("Unknown compatibility class entry");
        }
        let name = attribute(class, "fullyQualifiedName")?;
        if name.is_empty() || !seen.insert(name.to_owned()) {
            bail!("Duplicate or empty compatibility class");
        }
        let (binary, source) = (
            boolean(class, "binaryCompatible")?,
            boolean(class, "sourceCompatible")?,
        );
        all_binary &= binary;
        all_source &= source;
        let before = findings.len();
        for node in class.descendants().filter(Node::is_element) {
            if node.attribute("binaryCompatible").is_some()
                || node.attribute("sourceCompatible").is_some()
            {
                let (child_binary, child_source) = (
                    boolean(node, "binaryCompatible")?,
                    boolean(node, "sourceCompatible")?,
                );
                if (binary && !child_binary) || (source && !child_source) {
                    bail!("Compatibility class summary contradicts member evidence");
                }
                if node.has_tag_name("compatibilityChange")
                    && incompatible(child_binary, child_source, level)
                {
                    let change = attribute(node, "type")?;
                    if change.trim().is_empty() {
                        bail!("Empty compatibility change kind");
                    }
                    findings.push(Finding {
                        symbol: symbol(node, name)?,
                        change: change.into(),
                        binary_compatible: child_binary,
                        source_compatible: child_source,
                    });
                }
            }
        }
        if incompatible(binary, source, level) && findings.len() == before {
            findings.push(Finding {
                symbol: name.into(),
                change: "CLASS_INCOMPATIBLE".into(),
                binary_compatible: binary,
                source_compatible: source,
            });
        }
    }
    if seen.is_empty() || seen != *expected_classes {
        bail!("Compatibility report does not cover the complete nonempty archive class inventory");
    }
    findings.sort_by(|a, b| (&a.symbol, &a.change).cmp(&(&b.symbol, &b.change)));
    findings.dedup_by(|a, b| {
        a.symbol == b.symbol
            && a.change == b.change
            && a.binary_compatible == b.binary_compatible
            && a.source_compatible == b.source_compatible
    });
    Ok(Comparison {
        classes_compared: seen.len(),
        binary_compatible: all_binary,
        source_compatible: all_source,
        findings,
    })
}

/// Derive the public/protected class inventory from the verified, unfiltered run.
/// The second analyzer pass applies member visibility and compatibility semantics.
pub fn api_inventory(bytes: &[u8]) -> Result<BTreeSet<String>> {
    if bytes.len() > snapshot::MAX_FILE_BYTES {
        bail!("Compatibility report exceeds its byte budget");
    }
    let document = roxmltree::Document::parse(std::str::from_utf8(bytes)?)?;
    let classes = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("classes"))
        .context("Missing complete class inventory")?;
    let mut result = BTreeSet::new();
    for class in classes.children().filter(Node::is_element) {
        let modifiers = class
            .children()
            .find(|node| node.has_tag_name("modifiers"))
            .context("Missing class visibility inventory")?;
        let mut visibility = 0;
        let mut exported = false;
        for modifier in modifiers.children().filter(Node::is_element) {
            let old = attribute(modifier, "oldValue")?;
            let new = attribute(modifier, "newValue")?;
            let access =
                |value| ["PUBLIC", "PROTECTED", "PACKAGE_PROTECTED", "PRIVATE"].contains(&value);
            if access(old) || access(new) {
                if (!access(old) && old != "n.a.") || (!access(new) && new != "n.a.") {
                    bail!("Invalid class visibility transition");
                }
                visibility += 1;
                exported = [old, new]
                    .iter()
                    .any(|value| ["PUBLIC", "PROTECTED"].contains(value));
            }
        }
        if visibility != 1 {
            bail!("Class visibility must occur exactly once");
        }
        if exported {
            result.insert(attribute(class, "fullyQualifiedName")?.into());
        }
    }
    if result.is_empty() {
        bail!("Compatibility requires a nonempty public/protected API class inventory");
    }
    Ok(result)
}

#[cfg(test)]
#[path = "compatibility_tests.rs"]
mod tests;
