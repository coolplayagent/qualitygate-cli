//! Borrowed facts for one rule, acquired and bound by the application.

#[derive(Default)]
pub struct RuleFacts<'a> {
    pub syntax: Option<&'a crate::adapters::syntax::Cache>,
    pub projects: &'a [crate::domain::ProjectFacts],
    pub provenance: Option<&'a crate::adapters::provenance::ProvenanceFacts>,
    pub git_trailers: Option<&'a crate::adapters::git_trailers::GitFacts>,
}
