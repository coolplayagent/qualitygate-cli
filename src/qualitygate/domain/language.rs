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
