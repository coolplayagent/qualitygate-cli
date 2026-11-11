//! Shared syntax capability descriptors; availability does not establish project facts.

#[derive(Debug, serde::Serialize)]
pub struct Language {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub syntax_capabilities: &'static [&'static str],
    pub marker_options: &'static [&'static str],
}

pub const LANGUAGES: &[Language] = &[
    Language {
        name: "java",
        extensions: &["java"],
        syntax_capabilities: &["test_methods", "annotations", "comments", "imports"],
        marker_options: &["annotation", "comment", "git_trailer"],
    },
    Language {
        name: "python",
        extensions: &["py"],
        syntax_capabilities: &["test_methods", "annotations", "comments", "imports"],
        marker_options: &["annotation", "comment", "git_trailer"],
    },
    Language {
        name: "typescript",
        extensions: &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        syntax_capabilities: &["test_methods", "comments", "imports"],
        marker_options: &["comment", "git_trailer"],
    },
    Language {
        name: "go",
        extensions: &["go"],
        syntax_capabilities: &["test_methods", "comments", "imports"],
        marker_options: &["comment", "git_trailer"],
    },
    Language {
        name: "rust",
        extensions: &["rs"],
        syntax_capabilities: &["test_methods", "annotations", "comments", "imports"],
        marker_options: &["annotation", "comment", "git_trailer"],
    },
    Language {
        name: "shell",
        extensions: &["sh", "bash"],
        syntax_capabilities: &["comments"],
        marker_options: &[],
    },
];

pub fn named(name: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|language| language.name == name)
}

pub fn for_path(path: &str) -> Option<&'static Language> {
    let extension = std::path::Path::new(path).extension()?.to_str()?;
    LANGUAGES
        .iter()
        .find(|language| language.extensions.contains(&extension))
}

/// Extensionless executable scripts can declare the supported Shell grammar
/// in a bounded first-line interpreter directive.
pub fn for_source(path: &str, bytes: &[u8]) -> Option<&'static Language> {
    if let Some(language) = for_path(path) {
        return Some(language);
    }
    let directive = bytes
        .strip_prefix(b"#!")?
        .split(|byte| *byte == b'\n')
        .next()?;
    if directive.len() > 256 {
        return None;
    }
    let directive = std::str::from_utf8(directive).ok()?;
    let mut words = directive.split_whitespace();
    let interpreter = words.next()?.rsplit('/').next()?;
    let shell = if interpreter == "env" {
        words
            .find(|word| !word.starts_with('-'))?
            .rsplit('/')
            .next()?
    } else {
        interpreter
    };
    matches!(shell, "sh" | "bash")
        .then(|| named("shell"))
        .flatten()
}

/// Text-only review rules can cover C and C++ without claiming a syntax
/// adapter for either language. Headers use their conventional C/C++ suffix.
pub fn for_text_source(path: &str, bytes: &[u8]) -> Option<&'static str> {
    match std::path::Path::new(path)
        .extension()
        .and_then(|part| part.to_str())
    {
        Some("c" | "h") => Some("c"),
        Some("cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => Some("cpp"),
        _ => for_source(path, bytes).map(|language| language.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensionless_shell_shebang_is_recognized_without_reclassifying_other_files() {
        assert_eq!(
            for_source("bin/deploy", b"#!/usr/bin/env -S bash -e\necho ok\n")
                .unwrap()
                .name,
            "shell"
        );
        assert_eq!(
            for_source("bin/start", b"#!/bin/sh\necho ok\n")
                .unwrap()
                .name,
            "shell"
        );
        assert_eq!(
            for_source("bin/tool.py", b"#!/bin/sh\nprint(1)\n")
                .unwrap()
                .name,
            "python"
        );
        assert!(for_source("bin/tool", b"#!/usr/bin/env python\n").is_none());
        assert!(for_source("bin/tool", b"echo ok\n").is_none());
    }

    #[test]
    fn text_review_scope_keeps_c_family_out_of_syntax_capabilities() {
        for (path, expected) in [
            ("src/a.c", "c"),
            ("include/a.h", "c"),
            ("src/a.cc", "cpp"),
            ("src/a.cpp", "cpp"),
            ("src/a.cxx", "cpp"),
            ("include/a.hh", "cpp"),
            ("include/a.hpp", "cpp"),
            ("include/a.hxx", "cpp"),
        ] {
            assert_eq!(for_text_source(path, b""), Some(expected));
            assert!(for_source(path, b"").is_none());
        }
        assert_eq!(for_text_source("src/a.rs", b""), Some("rust"));
        assert_eq!(for_text_source("bin/run", b"#!/bin/sh\n"), Some("shell"));
    }
}
