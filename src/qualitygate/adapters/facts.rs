//! Borrowed facts for one rule, acquired and bound by the application.

#[derive(Default)]
pub struct RuleFacts<'a> {
    pub projects: &'a [crate::domain::ProjectFacts],
    pub provenance: Option<&'a super::provenance::ProvenanceFacts>,
    pub git_trailers: Option<&'a super::git_trailers::GitFacts>,
}
