//! Bound scalar resolution work before Serde interprets numeric aliases.
//! Event scalars retain their text; anchor costs are reused without expansion.

use saphyr_parser::{Event, Marker, Parser, Tag};
use std::{collections::BTreeMap, fmt};

const MAX_SCALAR_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODE_EVENTS: usize = 100_000;

#[derive(Debug)]
pub(super) struct Error {
    message: String,
    pub(super) location: Option<(usize, usize)>,
}

impl Error {
    fn new(message: impl Into<String>, mark: Option<Marker>) -> Self {
        Self {
            message: message.into(),
            location: mark.map(|mark| (mark.line(), mark.col() + 1)),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)?;
        if let Some((line, column)) = self.location {
            write!(formatter, " at line {line} column {column}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

struct Frame {
    anchor: usize,
    start: usize,
}

#[derive(Default)]
struct Budget {
    total: usize,
    nodes: usize,
    frames: Vec<Frame>,
    anchors: BTreeMap<usize, Option<usize>>,
}

impl Budget {
    fn charge(&mut self, bytes: usize, mark: Marker) -> Result<(), Error> {
        self.total = self
            .total
            .checked_add(bytes)
            .filter(|total| *total <= MAX_SCALAR_TEXT_BYTES)
            .ok_or_else(|| {
                Error::new(
                    "Expanded YAML scalar text exceeds 16777216 bytes",
                    Some(mark),
                )
            })?;
        Ok(())
    }

    fn depth(&self, mark: Marker) -> Result<(), Error> {
        if self.frames.len() > MAX_DEPTH {
            return Err(Error::new("YAML nesting exceeds 64 levels", Some(mark)));
        }
        Ok(())
    }

    fn tag(&mut self, tag: Option<&Tag>, mark: Marker) -> Result<(), Error> {
        if let Some(tag) = tag {
            self.charge(tag.handle.len(), mark)?;
            self.charge(tag.suffix.len(), mark)?;
        }
        Ok(())
    }

    fn event(&mut self, event: Event<'_>, mark: Marker) -> Result<(), Error> {
        if matches!(
            event,
            Event::Scalar(..)
                | Event::SequenceStart(..)
                | Event::MappingStart(..)
                | Event::Alias(_)
        ) {
            // Raw nodes are a lower bound on expanded nodes. Bound the event
            // work and anchor cache before the later expanded-value traversal.
            self.nodes += 1;
            if self.nodes > MAX_NODE_EVENTS {
                return Err(Error::new("Expanded YAML exceeds 100000 nodes", Some(mark)));
            }
        }
        match event {
            Event::Scalar(text, _, anchor, tag) => {
                self.depth(mark)?;
                let start = self.total;
                self.charge(text.len(), mark)?;
                self.tag(tag.as_deref(), mark)?;
                if anchor != 0 {
                    self.anchors.insert(anchor, Some(self.total - start));
                }
            }
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                self.depth(mark)?;
                let start = self.total;
                self.tag(tag.as_deref(), mark)?;
                if anchor != 0 {
                    self.anchors.insert(anchor, None);
                }
                self.frames.push(Frame { anchor, start });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let frame = self
                    .frames
                    .pop()
                    .ok_or_else(|| Error::new("Unexpected YAML collection end", Some(mark)))?;
                if frame.anchor != 0 {
                    self.anchors
                        .insert(frame.anchor, Some(self.total - frame.start));
                }
            }
            Event::Alias(anchor) => {
                self.depth(mark)?;
                let bytes = self
                    .anchors
                    .get(&anchor)
                    .ok_or_else(|| Error::new("YAML alias has no anchor", Some(mark)))?;
                let bytes = bytes.ok_or_else(|| {
                    Error::new(
                        "YAML nesting exceeds 64 levels through a recursive alias",
                        Some(mark),
                    )
                })?;
                self.charge(bytes, mark)?;
            }
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
            Event::Nothing | Event::DocumentStart(_) => {
                return Err(Error::new("Unexpected YAML parser event", Some(mark)));
            }
        }
        Ok(())
    }
}

pub(super) fn validate(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > super::MAX_CONFIG_BYTES {
        return Err(Error::new("YAML input exceeds 1048576 bytes", None));
    }
    // The existing Serde decoder explicitly selects UTF-8, including its BOM.
    let input =
        std::str::from_utf8(bytes).map_err(|_| Error::new("Invalid UTF-8 YAML input", None))?;
    let mut budget = Budget::default();
    let mut document = false;
    for event in Parser::new_from_str(input) {
        let (event, span) =
            event.map_err(|error| Error::new(error.info(), Some(*error.marker())))?;
        if matches!(event, Event::DocumentStart(_)) {
            if document {
                return Err(Error::new(
                    "Multiple YAML documents are not supported",
                    Some(span.start),
                ));
            }
            document = true;
        } else {
            budget.event(event, span.start)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_text_budget_counts_numeric_and_utf8_aliases_before_resolution() {
        for scalar in [
            "-0x".to_owned() + &"0".repeat(131_068) + "1",
            format!("'{}xx'", "界".repeat(43_690)),
        ] {
            let exact = format!("[&a {scalar}, {}]", vec!["*a"; 127].join(","));
            validate(exact.as_bytes()).unwrap();
            let over = exact.replacen("]", ",*a]", 1);
            let error = validate(over.as_bytes()).unwrap_err();
            assert!(error.to_string().contains("scalar text exceeds 16777216"));
            assert_eq!(error.location.unwrap().0, 1);
        }
    }

    #[test]
    fn scalar_text_budget_reuses_nested_anchor_costs_and_charges_keys_and_tags() {
        let prefix = format!(
            "one: &one '{}'\ntwo: &two [{}]\nthree: [",
            "x".repeat(16_384),
            vec!["*one"; 16].join(",")
        );
        validate(format!("{prefix}{}]", vec!["*two"; 62].join(",")).as_bytes()).unwrap();
        assert!(
            validate(format!("{prefix}{}]", vec!["*two"; 63].join(",")).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("scalar text exceeds")
        );
        let map = format!(
            "[&a {{ ? '{}' : 0 }}, {}]",
            "x".repeat(131_071),
            vec!["*a"; 127].join(",")
        );
        validate(map.as_bytes()).unwrap();
        assert!(
            validate(map.replacen("]", ",*a]", 1).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("scalar text exceeds")
        );
        let tagged = format!(
            "[&a !<tag:example.org,2026:{}> x, {}]",
            "a".repeat(8192),
            vec!["*a"; 2048].join(",")
        );
        assert!(
            validate(tagged.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("scalar text exceeds")
        );
    }

    #[test]
    fn scalar_preflight_preserves_yaml_encodings_tags_and_aliases_and_bounds_failures() {
        let input = "[&a !custom {x: '界😀'}, *a, &a 2, *a]";
        let expected: serde_norway::Value = serde_norway::from_str(input).unwrap();
        let encodings = [
            input.as_bytes().to_vec(),
            format!("\u{feff}{input}").into_bytes(),
        ];
        for bytes in encodings {
            assert_eq!(crate::config::yaml_budget::value(&bytes).unwrap(), expected);
        }
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in input.encode_utf16() {
                bytes.extend(if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            assert!(serde_norway::from_slice::<serde_norway::Value>(&bytes).is_err());
            assert!(crate::config::yaml_budget::value(&bytes).is_err());
        }
        for valid in [
            "",
            "null",
            "%YAML 1.1\n---\n[true, 1, 1.5]",
            "|\n  *a &a 0x0001\n",
        ] {
            validate(valid.as_bytes()).unwrap();
        }
        for invalid in ["[", "*missing", "---\na\n---\nb\n"] {
            assert!(validate(invalid.as_bytes()).is_err());
        }
        for bytes in [&[0xff][..], &[0xff, 0xfe, 0], &[0xfe, 0xff, 0xd8, 0]] {
            assert!(validate(bytes).is_err());
            assert!(serde_norway::from_slice::<serde_norway::Value>(bytes).is_err());
        }
        for (input, reason) in [
            ("&a [*a]".to_owned(), "nesting exceeds 64 levels"),
            (
                format!("{}null{}", "[".repeat(65), "]".repeat(65)),
                "nesting exceeds 64 levels",
            ),
            // The scanner rejects this input before emitting node events.
            (
                format!("{}null{}", "[".repeat(2000), "]".repeat(2000)),
                "recursion limit exceeded",
            ),
        ] {
            let error = validate(input.as_bytes()).unwrap_err();
            assert!(error.to_string().contains(reason), "{error}");
        }
        let nodes = format!("[{}]", vec!["0"; MAX_NODE_EVENTS - 1].join(","));
        validate(nodes.as_bytes()).unwrap();
        assert!(
            validate(nodes.replacen("]", ",0]", 1).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("100000 nodes")
        );
        assert!(
            validate(&vec![b' '; super::super::MAX_CONFIG_BYTES + 1])
                .unwrap_err()
                .location
                .is_none()
        );
        assert!(crate::config::yaml_budget::value(b"{same: 1, same: 2}").is_err());
    }

    #[test]
    fn scalar_budget_errors_keep_configuration_diagnostics_and_source_locations() {
        let input = format!(
            "large: &a -0x{}1\nextra: [{}]\n",
            "0".repeat(131_072),
            vec!["*a"; 128].join(",")
        );
        let error =
            crate::config::runtime::decode::<serde_norway::Value>(input.as_bytes()).unwrap_err();
        let error = error
            .downcast_ref::<crate::domain::runtime::ConfigurationError>()
            .unwrap();
        assert_eq!(error.code, "config.syntax");
        assert_eq!(error.line, Some(2));
        assert!(error.column.unwrap() > 1);
        assert!(error.message.contains("scalar text exceeds 16777216 bytes"));
    }
}
