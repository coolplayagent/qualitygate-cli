//! Borrowed facts for one rule, acquired and bound by the application.

pub struct RuleFacts<'a, P = crate::domain::ProjectFacts> {
    pub tool_facts: Option<
        &'a std::collections::BTreeMap<
            String,
            std::sync::Arc<crate::tool_evidence::contracts::ToolFacts>,
        >,
    >,
    pub syntax: Option<&'a crate::adapters::syntax::Cache>,
    pub projects: &'a [P],
    pub provenance: Option<&'a crate::adapters::provenance::ProvenanceFacts>,
    pub git_trailers: Option<&'a crate::adapters::git_trailers::GitFacts>,
}

pub type BorrowedRuleFacts<'a> = RuleFacts<'a, &'a crate::domain::ProjectFacts>;

impl<P> Default for RuleFacts<'_, P> {
    fn default() -> Self {
        Self {
            projects: &[],
            tool_facts: None,
            syntax: None,
            provenance: None,
            git_trailers: None,
        }
    }
}

impl RuleFacts<'_> {
    pub(crate) fn with_borrowed_projects<R>(
        &self,
        evaluate: impl FnOnce(&BorrowedRuleFacts<'_>) -> R,
    ) -> R {
        let projects: Vec<_> = self.projects.iter().collect();
        evaluate(&RuleFacts {
            projects: &projects,
            tool_facts: self.tool_facts,
            syntax: self.syntax,
            provenance: self.provenance,
            git_trailers: self.git_trailers,
        })
    }
}
