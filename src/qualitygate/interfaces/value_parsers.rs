//! CLI spellings and possible-value help stay outside reusable domain contracts.
use crate::domain::{Severity, evolution::ActorKind};
use clap::{ValueEnum, builder::TypedValueParser};

#[derive(Clone, ValueEnum)]
enum SeverityArg {
    Error,
    Warning,
    Info,
}

#[derive(Clone, ValueEnum)]
enum ActorKindArg {
    Human,
    Agent,
}

pub(super) fn severity() -> impl TypedValueParser<Value = Severity> {
    clap::builder::EnumValueParser::<SeverityArg>::new().map(|value| match value {
        SeverityArg::Error => Severity::Error,
        SeverityArg::Warning => Severity::Warning,
        SeverityArg::Info => Severity::Info,
    })
}

pub(super) fn actor_kind() -> impl TypedValueParser<Value = ActorKind> {
    clap::builder::EnumValueParser::<ActorKindArg>::new().map(|value| match value {
        ActorKindArg::Human => ActorKind::Human,
        ActorKindArg::Agent => ActorKind::Agent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[derive(Debug, Parser)]
    struct Options {
        #[arg(long, value_parser = severity())]
        severity: Option<Severity>,
        #[arg(long, value_parser = actor_kind(), default_value = "agent")]
        actor_kind: ActorKind,
    }

    #[test]
    fn cli_values_keep_domain_values_defaults_and_serialization() {
        let defaults = Options::try_parse_from(["test"]).unwrap();
        assert_eq!(defaults.severity, None);
        assert_eq!(defaults.actor_kind, ActorKind::Agent);
        for (argument, expected) in [
            ("error", Severity::Error),
            ("warning", Severity::Warning),
            ("info", Severity::Info),
        ] {
            let options = Options::try_parse_from(["test", "--severity", argument]).unwrap();
            assert_eq!(options.severity, Some(expected));
            assert_eq!(serde_json::to_value(expected).unwrap(), argument);
        }
        for (argument, expected) in [("human", ActorKind::Human), ("agent", ActorKind::Agent)] {
            let options = Options::try_parse_from(["test", "--actor-kind", argument]).unwrap();
            assert_eq!(options.actor_kind, expected);
            assert_eq!(serde_json::to_value(expected).unwrap(), argument);
        }
    }

    #[test]
    fn cli_help_and_invalid_values_retain_the_enumerated_contract() {
        let help = Options::command().render_long_help().to_string();
        assert!(help.contains("possible values: error, warning, info"));
        assert!(help.contains("possible values: human, agent"));
        for (flag, value, choices) in [
            ("--severity", "fatal", "error, warning, info"),
            ("--severity", "Warning", "error, warning, info"),
            ("--actor-kind", "robot", "human, agent"),
            ("--actor-kind", "Human", "human, agent"),
        ] {
            let error = Options::try_parse_from(["test", flag, value]).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
            assert!(error.to_string().contains(choices));
        }
    }
}
