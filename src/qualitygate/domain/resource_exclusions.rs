//! Pure, versioned default document/media exclusions and their coverage evidence.
use serde::{Deserialize, Serialize};

pub const CLASSIFIER_VERSION: u32 = 1;
pub const EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "png", "jpg",
    "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "avif", "heic", "heif", "mp3", "wav",
    "flac", "aac", "ogg", "m4a", "mp4", "mov", "avi", "mkv", "webm", "mpeg", "mpg",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub enabled: bool,
    pub retain: Vec<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: true,
            retain: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Explicit,
    UnstructuredDocument,
}

pub fn reason(path: &str, enabled: bool, explicit: bool, retained: bool) -> Option<Reason> {
    if explicit {
        return Some(Reason::Explicit);
    }
    let extension = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'));
    (enabled
        && !retained
        && extension.is_some_and(|(_, ext)| {
            EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        }))
    .then_some(Reason::UnstructuredDocument)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub classifier_version: u32,
    pub policy: Policy,
    pub protected_paths: Vec<String>,
    pub default_excluded_count: usize,
    pub default_excluded_paths: Vec<String>,
    pub explicit_excluded_count: usize,
    pub explicit_excluded_paths: Vec<String>,
    pub details_truncated: bool,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            classifier_version: CLASSIFIER_VERSION,
            policy: Policy::default(),
            protected_paths: Vec::new(),
            default_excluded_count: 0,
            default_excluded_paths: Vec::new(),
            explicit_excluded_count: 0,
            explicit_excluded_paths: Vec::new(),
            details_truncated: false,
        }
    }
}

impl Evidence {
    pub fn record(&mut self, path: &str, reason: Reason, limit: usize) {
        let (count, paths) = match reason {
            Reason::Explicit => (
                &mut self.explicit_excluded_count,
                &mut self.explicit_excluded_paths,
            ),
            Reason::UnstructuredDocument => (
                &mut self.default_excluded_count,
                &mut self.default_excluded_paths,
            ),
        };
        *count += 1;
        if paths.len() < limit {
            paths.push(path.into());
        } else {
            self.details_truncated = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_is_case_insensitive_and_bounded_to_known_suffixes() {
        for extension in EXTENSIONS {
            for path in [
                format!("asset.{extension}"),
                format!("a/b/asset.{}", extension.to_ascii_uppercase()),
            ] {
                assert_eq!(
                    reason(&path, true, false, false),
                    Some(Reason::UnstructuredDocument)
                );
                assert_eq!(reason(&path, false, false, false), None);
                assert_eq!(reason(&path, true, false, true), None);
                assert_eq!(reason(&path, true, true, true), Some(Reason::Explicit));
            }
        }
        for path in [
            "doc.md",
            "page.html",
            "logo.svg",
            "font.woff",
            "archive.zip",
            "legacy.bin",
            "no-extension",
            "dir.pdf/source.rs",
            "doc.pdf.rs",
            "doc.pdf/",
        ] {
            assert_eq!(reason(path, true, false, false), None, "{path}");
        }
    }

    #[test]
    fn evidence_counts_remain_complete_when_details_are_bounded() {
        let mut evidence = Evidence::default();
        evidence.record("a.pdf", Reason::UnstructuredDocument, 1);
        evidence.record("b.pdf", Reason::UnstructuredDocument, 1);
        evidence.record("c.bin", Reason::Explicit, 1);
        assert_eq!(evidence.default_excluded_count, 2);
        assert_eq!(evidence.default_excluded_paths, ["a.pdf"]);
        assert_eq!(evidence.explicit_excluded_count, 1);
        assert!(evidence.details_truncated);
        assert_eq!(evidence.classifier_version, CLASSIFIER_VERSION);
    }
}
