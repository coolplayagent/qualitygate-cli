use anyhow::{Context, Result, bail};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

#[derive(Default)]
struct Document {
    anchors: BTreeSet<String>,
    links: Vec<(String, usize)>,
    errors: Vec<String>,
}

impl Document {
    fn parse(text: &str) -> Self {
        let mut result = Self::default();
        let mut heading = None::<String>;
        let mut fence = None::<(usize, usize, usize)>;
        let mut broken = Vec::new();
        let mut callback = |link: pulldown_cmark::BrokenLink<'_>| {
            broken.push(format!("undefined reference [{}]", link.reference));
            None
        };
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        let parser = Parser::new_with_broken_link_callback(text, options, Some(&mut callback));
        let punctuation = regex::Regex::new(r"[^\p{L}\p{N}\p{M}_ \-]").unwrap();
        let html_tag =
            regex::Regex::new(r#"(?s)<[A-Za-z][A-Za-z0-9:-]*(?:[^<>"']|"[^"]*"|'[^']*')*>"#)
                .unwrap();
        let html_attribute = regex::Regex::new(
            r#"(?i)\b([a-z_:][-a-z0-9_:.]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#,
        )
        .unwrap();
        let line_starts: Vec<_> = std::iter::once(0)
            .chain(
                text.bytes()
                    .enumerate()
                    .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
            )
            .collect();
        for (event, range) in parser.into_offset_iter() {
            let line = line_starts.partition_point(|start| *start <= range.start);
            match event {
                Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
                Event::Text(value) | Event::Code(value) if heading.is_some() => {
                    heading.as_mut().unwrap().push_str(&value);
                }
                Event::Text(_) if fence.is_some() => {
                    fence.as_mut().unwrap().2 = range.end;
                }
                Event::SoftBreak | Event::HardBreak if heading.is_some() => {
                    heading.as_mut().unwrap().push(' ');
                }
                Event::End(TagEnd::Heading(_)) => {
                    let text = heading.take().unwrap();
                    let base = punctuation
                        .replace_all(&text.to_lowercase(), "")
                        .replace(' ', "-");
                    let mut anchor = base.clone();
                    let mut suffix = 0;
                    while result.anchors.contains(&anchor) {
                        suffix += 1;
                        anchor = format!("{base}-{suffix}");
                    }
                    result.anchors.insert(anchor);
                }
                Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                    result.links.push((dest_url.into_string(), line));
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                    let content_start = text[range.clone()]
                        .find('\n')
                        .map_or(range.end, |offset| range.start + offset + 1);
                    fence = Some((range.start, line, content_start));
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some((start, line, content_end)) = fence.take()
                        && (content_end >= range.end || !closed_fence(&text[start..range.end]))
                    {
                        result
                            .errors
                            .push(format!("line {line}: unclosed code fence"));
                    }
                }
                Event::Html(value) | Event::InlineHtml(value) => {
                    if value.trim_start().starts_with("<!--") {
                        continue;
                    }
                    for capture in html_tag
                        .find_iter(&value)
                        .flat_map(|tag| html_attribute.captures_iter(tag.as_str()))
                    {
                        if !matches!(
                            capture[1].to_ascii_lowercase().as_str(),
                            "id" | "name" | "href" | "src"
                        ) {
                            continue;
                        }
                        let target = capture
                            .get(2)
                            .or(capture.get(3))
                            .or(capture.get(4))
                            .unwrap()
                            .as_str();
                        if target.contains('&') {
                            result.errors.push(format!(
                                "line {line}: HTML link/anchor entities need explicit support"
                            ));
                            continue;
                        }
                        if matches!(capture[1].to_ascii_lowercase().as_str(), "href" | "src") {
                            result.links.push((target.into(), line));
                        } else if !result.anchors.insert(target.into()) {
                            result
                                .errors
                                .push(format!("line {line}: duplicate explicit anchor {target}"));
                        }
                    }
                }
                _ => {}
            }
        }
        result.errors.extend(broken);
        result
    }
}

fn closed_fence(block: &str) -> bool {
    let mut lines = block.lines();
    let opening = lines.next().unwrap_or_default().trim_start();
    let Some(marker @ ('~' | '`')) = opening.chars().next() else {
        return false;
    };
    let width = opening
        .chars()
        .take_while(|character| *character == marker)
        .count();
    let Some(mut closing) = lines.next_back().map(str::trim_start) else {
        return false;
    };
    while closing.starts_with('>') {
        closing = closing[1..].trim_start();
    }
    closing
        .chars()
        .take_while(|character| *character == marker)
        .count()
        >= width
        && closing.trim_start_matches(marker).trim().is_empty()
}

fn decode(value: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut input = value.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let a = input
                .next()
                .and_then(|value| (value as char).to_digit(16))
                .context("invalid percent escape")?;
            let b = input
                .next()
                .and_then(|value| (value as char).to_digit(16))
                .context("invalid percent escape")?;
            bytes.push((a * 16 + b) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).context("link is not UTF-8")
}

fn resolve(file: &Path, target: &str) -> Result<Option<(PathBuf, String)>> {
    if target.starts_with("//") {
        return Ok(None);
    }
    if let Ok(url) = reqwest::Url::parse(target) {
        if ["http", "https", "mailto", "data"].contains(&url.scheme()) {
            return Ok(None);
        }
        bail!("unsupported link scheme: {}", url.scheme());
    }
    let (path, fragment) = target.split_once('#').unwrap_or((target, ""));
    let path = decode(path.split('?').next().unwrap())?;
    let fragment = decode(fragment)?;
    if path.contains(['\\', '\0']) || fragment.contains('\0') {
        bail!("invalid link path")
    }
    let base = if path.is_empty() {
        file.to_path_buf()
    } else if path.starts_with('/') {
        PathBuf::from(path.trim_start_matches('/'))
    } else {
        file.parent().unwrap().join(path)
    };
    let mut normalized = PathBuf::new();
    for part in base.components() {
        match part {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => bail!("link escapes repository"),
        }
    }
    Ok(Some((normalized, fragment)))
}

fn validate(root: &Path, documents: &BTreeMap<PathBuf, Document>) -> Vec<String> {
    let mut errors = Vec::new();
    for (file, document) in documents {
        errors.extend(
            document
                .errors
                .iter()
                .map(|error| format!("{}: {error}", file.display())),
        );
        for (target, line) in &document.links {
            let failure = match resolve(file, target) {
                Ok(None) => None,
                Err(error) => Some(error.to_string()),
                Ok(Some((path, fragment))) => match dunce::canonicalize(root.join(&path)) {
                    Err(_) => Some("missing local target".into()),
                    Ok(absolute) if !absolute.starts_with(root) => {
                        Some("link escapes repository".into())
                    }
                    Ok(_) if fragment.is_empty() => None,
                    Ok(_) => match documents.get(&path) {
                        Some(target) if target.anchors.contains(&fragment) => None,
                        Some(_) => Some(format!("missing Markdown anchor #{fragment}")),
                        None => {
                            Some("fragments on non-Markdown targets need explicit support".into())
                        }
                    },
                },
            };
            if let Some(error) = failure {
                errors.push(format!("{}:{line}: {target}: {error}", file.display()));
            }
        }
    }
    errors
}

#[test]
fn documentation_links_anchors_and_fences_are_valid() {
    let root = dunce::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
    let documents = super::files(&root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            (
                path.strip_prefix(&root).unwrap().to_path_buf(),
                Document::parse(&text),
            )
        })
        .collect();
    let errors = validate(&root, &documents);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn documentation_resolves_rendered_headings_references_and_escaped_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(temporary.path()).unwrap();
    std::fs::write(root.join("Index.md"), "").unwrap();
    std::fs::create_dir(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/My Guide.md"), "").unwrap();
    let index = Document::parse(
        r##"# Intro
[guide](docs/My%20Guide.md#测试-api)
[again][reference]
[here](#intro)
[named](docs/My%20Guide.md#explicit)
[titled](<docs/My Guide.md> "Guide title")
<a href="docs/My%20Guide.md#repeat-1">Repeat</a>

[reference]: docs/My%20Guide.md#repeat-2
~~~markdown
[example](missing.md)
# Not a heading
~~~
"##,
    );
    let guide = Document::parse(
        "# 测试 *API*\n\nRepeat\n======\n\n## Repeat-1\n\n## Repeat\n\n<a name=\"explicit\"></a>\n",
    );
    assert!(guide.anchors.contains("测试-api"));
    assert!(guide.anchors.contains("repeat-2"));
    let html = Document::parse("<div title=\"id='fake' href='missing.md'\">name=also-fake</div>\n");
    assert!(html.anchors.is_empty());
    assert!(html.links.is_empty());
    let documents = BTreeMap::from([
        (PathBuf::from("Index.md"), index),
        (PathBuf::from("docs/My Guide.md"), guide),
    ]);
    assert!(
        validate(&root, &documents).is_empty(),
        "{:?}",
        validate(&root, &documents)
    );
}

#[test]
fn documentation_rejects_missing_anchors_references_targets_and_open_fences() {
    let temporary = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(temporary.path()).unwrap();
    std::fs::write(root.join("README.md"), "").unwrap();
    let text = "# Existing\n\n[bad](#missing)\n[no file](no.md)\n[bad reference][undefined]\n[escape](../outside.md)\n[bad percent](%ZZ.md)\n\n````rust\n```\n";
    let errors = validate(
        &root,
        &BTreeMap::from([(PathBuf::from("README.md"), Document::parse(text))]),
    )
    .join("\n");
    for expected in [
        "missing Markdown anchor",
        "missing local target",
        "undefined reference",
        "escapes repository",
        "invalid percent escape",
        "unclosed code fence",
    ] {
        assert!(errors.contains(expected), "{expected}: {errors}");
    }
    for text in [
        "~~~\nopen",
        "```\n",
        "> ```rust\n> open\n",
        "```rust\n    ```\n",
        "```rust\n\t```\n",
    ] {
        assert!(!Document::parse(text).errors.is_empty(), "{text}");
    }
    for text in [
        "````rust\n```\n````\n",
        "> ```rust\n> code\n> ```\n",
        "~~~\ncode\n~~~~\n",
        "- list\n\n  ```rust\n  body\n  ```\n",
    ] {
        assert!(Document::parse(text).errors.is_empty(), "{text}");
    }
}
