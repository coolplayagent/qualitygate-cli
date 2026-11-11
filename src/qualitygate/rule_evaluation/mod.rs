//! Evaluates builtin and custom rules over captured facts.

pub(crate) mod builtin_conventions;
pub mod builtin_rules;
pub mod custom_rules;
pub mod evaluation_facts;
pub(crate) mod language_syntax;
pub(crate) mod markers;
pub(crate) mod native_syntax;
pub mod operators_v2;
pub mod operators_v3;
pub(crate) mod project_checks;
pub(crate) mod shell_conventions;
pub(crate) mod structure_rules;
