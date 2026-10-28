//! Account for expanded YAML before allocating Values or typed collections.
//! Alias visits consume the same budget as their referenced values.

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use std::fmt;

const MAX_NODES: usize = 100_000;
const MAX_STRING_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

pub(super) fn value(bytes: &[u8]) -> Result<serde_norway::Value, serde_norway::Error> {
    validate(bytes)?;
    // Retain the existing Value visitor's duplicate-key and YAML semantics.
    serde_norway::from_slice(bytes)
}

fn validate(bytes: &[u8]) -> Result<(), serde_norway::Error> {
    if bytes.len() > super::MAX_CONFIG_BYTES {
        return Err(de::Error::custom("YAML input exceeds 1048576 bytes"));
    }
    Seed {
        budget: &mut Budget::default(),
        depth: 0,
    }
    .deserialize(serde_norway::Deserializer::from_slice(bytes))
}

#[derive(Default)]
struct Budget {
    nodes: usize,
    string_bytes: usize,
}

struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if self.depth > MAX_DEPTH {
            return Err(de::Error::custom("YAML nesting exceeds 64 levels"));
        }
        self.budget.nodes += 1;
        if self.budget.nodes > MAX_NODES {
            return Err(de::Error::custom("Expanded YAML exceeds 100000 nodes"));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded YAML value")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        self.budget.string_bytes = self
            .budget
            .string_bytes
            .checked_add(value.len())
            .filter(|bytes| *bytes <= MAX_STRING_BYTES)
            .ok_or_else(|| E::custom("Expanded YAML strings exceed 4194304 bytes"))?;
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence
            .next_element_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<(), A::Error> {
        while mapping
            .next_key_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?
            .is_some()
        {
            mapping.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
        }
        Ok(())
    }

    fn visit_enum<A: EnumAccess<'de>>(self, value: A) -> Result<(), A::Error> {
        let (_, variant) = value.variant_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })?;
        variant.newtype_variant_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_yaml_decoders_guard_suites_trust_and_duplicate_keys() {
        let input = format!(
            "large: &a '{}'\nextra: [{}]\n",
            "x".repeat(131_072),
            vec!["*a"; 32].join(",")
        );
        let suite = crate::config::policy_acceptance::parse_suite(input.as_bytes()).unwrap_err();
        let trust = crate::config::parse_yaml::<crate::config::policy_acceptance::EvolutionTrust>(
            input.as_bytes(),
        )
        .unwrap_err();
        for error in [suite, trust] {
            assert!(error.to_string().contains("strings exceed 4194304 bytes"));
        }
        assert!(value(b"outer: {same: 1, same: 2}").is_err());
        let parsed: serde_norway::Value =
            crate::config::parse_yaml(b"[&a {key: value}, *a]").unwrap();
        assert_eq!(parsed[0], parsed[1]);
    }

    #[test]
    fn yaml_expansion_charges_string_aliases_and_utf8_bytes_before_allocation() {
        // Root sequence, one anchored string and 31 aliases total exactly 4 MiB.
        for text in ["x".repeat(131_072), "界".repeat(43_690) + "xx"] {
            let exact = format!("[&a '{text}', {}]", vec!["*a"; 31].join(","));
            validate(exact.as_bytes()).unwrap();
            let over = exact.replacen("]", ",*a]", 1);
            assert!(
                validate(over.as_bytes())
                    .unwrap_err()
                    .to_string()
                    .contains("strings exceed")
            );
        }
        let literal = "[&a '*a', '*a', '&a']";
        validate(literal.as_bytes()).unwrap();
    }

    #[test]
    fn yaml_expansion_counts_nodes_and_mapping_keys_across_aliases() {
        let exact = format!("[{}]", vec!["0"; MAX_NODES - 1].join(","));
        validate(exact.as_bytes()).unwrap();
        assert!(
            validate(exact.replacen("]", ",0]", 1).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("100000 nodes")
        );
        let map = format!("[&a {{k: null}}, {}]", vec!["*a"; 33_332].join(","));
        validate(map.as_bytes()).unwrap();
        assert!(
            validate(map.replacen("]", ",*a]", 1).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("100000 nodes")
        );
    }

    #[test]
    fn yaml_expansion_keeps_scalar_tag_and_syntax_semantics_and_bounds_depth() {
        for valid in [
            "",
            "null",
            "[true, false, -1, 18446744073709551615, 1.5, .inf, .nan]",
            "!custom [1, {x: value}]",
            "{? [one, two]: value}",
            "|\n  *a &b [literal]\n",
        ] {
            validate(valid.as_bytes()).unwrap();
        }
        let nested = |depth: usize| format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        validate(nested(MAX_DEPTH).as_bytes()).unwrap();
        for text in [nested(MAX_DEPTH + 1), nested(2_000), "&a [*a]".to_owned()] {
            assert!(
                validate(text.as_bytes())
                    .unwrap_err()
                    .to_string()
                    .contains("nesting exceeds")
            );
        }
        for invalid in ["[", "*absent", "---\na\n---\nb\n"] {
            assert!(validate(invalid.as_bytes()).is_err());
        }
        assert!(
            validate(&vec![b' '; super::super::MAX_CONFIG_BYTES + 1])
                .unwrap_err()
                .to_string()
                .contains("input exceeds")
        );
    }
}
