//! Python project evidence requires an actual isolated, complete installation.

use super::{CommandCheck, ProjectSpec, PythonProject};
use anyhow::{Result, bail};
use std::{collections::BTreeSet, path::Path};

pub(super) fn validate(check: &CommandCheck) -> Result<()> {
    for project in &check.projects {
        let ProjectSpec::Python(project) = project else {
            continue;
        };
        if check.projects.len() != 1 || check.cwd != project.root {
            bail!("Python installation needs one project and cwd equal to its root");
        }
        for path in [
            &project.source_root,
            &project.test_source_root,
            &project.install_target,
        ] {
            let normalized = crate::paths::relative(Path::new(path))?;
            if normalized.is_empty()
                || normalized != *path
                || (project.root != "."
                    && !path
                        .strip_prefix(&project.root)
                        .is_some_and(|rest| rest.starts_with('/')))
            {
                bail!(
                    "Python source/test/install roots must be normalized subdirectories of the project"
                );
            }
        }
        if overlap(&project.source_root, &project.test_source_root)
            || [
                &project.source_root,
                &project.test_source_root,
                &project.install_report,
            ]
            .iter()
            .any(|path| overlap(path, &project.install_target))
        {
            bail!("Python install, source, test and report paths must not overlap");
        }
        let mut extras = BTreeSet::new();
        if project.extras.len() > 64 {
            bail!("Python extras exceed 64 entries");
        }
        for extra in &project.extras {
            let name: pep508_rs::ExtraName = extra.parse()?;
            if name.as_ref() != extra || !extras.insert(extra) {
                bail!("Python extras must be unique normalized names");
            }
        }
        if check.argv.len() < 8
            || check.argv[1..7] != ["-E", "-P", "-m", "pip", "--isolated", "install"]
        {
            bail!("Python evidence requires python -E -P -m pip --isolated install");
        }
        let expected = if extras.is_empty() {
            ".".into()
        } else {
            format!(
                ".[{}]",
                extras.into_iter().cloned().collect::<Vec<_>>().join(",")
            )
        };
        let mut target = None;
        let mut report = None;
        let mut ignored = false;
        let mut requested = false;
        let mut args = check.argv[7..].iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--ignore-installed" if !ignored => ignored = true,
                "--target" if target.is_none() => target = args.next(),
                "--report" if report.is_none() => report = args.next(),
                "--no-compile"
                | "--no-warn-script-location"
                | "--disable-pip-version-check"
                | "--no-input"
                | "--no-cache-dir" => {}
                "--index-url" | "--extra-index-url" => {
                    if args.next().is_none() {
                        bail!("Python index URL is missing");
                    }
                }
                value if value == expected && !requested => requested = true,
                _ => bail!("Unsupported or duplicate pip evidence argument: {arg}"),
            }
        }
        if !ignored
            || !requested
            || target.map(String::as_str) != Some(local(project, &project.install_target)?)
            || report.map(String::as_str) != Some(local(project, &project.install_report)?)
        {
            bail!(
                "Python evidence needs ignore-installed, matching target/report and exactly the configured project extras"
            );
        }
        for (id, suffix) in [
            ("python", vec!["-E", "-P", "--version"]),
            ("pip", vec!["-E", "-P", "-m", "pip", "--version"]),
        ] {
            if !check.tools.iter().any(|tool| {
                tool.id == id
                    && tool.argv.first() == check.argv.first()
                    && tool
                        .argv
                        .iter()
                        .skip(1)
                        .map(String::as_str)
                        .eq(suffix.iter().copied())
            }) {
                bail!(
                    "Python evidence needs a {id} version probe using the installation interpreter"
                );
            }
        }
    }
    Ok(())
}

fn local<'a>(project: &PythonProject, path: &'a str) -> Result<&'a str> {
    if project.root == "." {
        Ok(path)
    } else {
        path.strip_prefix(&project.root)
            .and_then(|rest| rest.strip_prefix('/'))
            .ok_or_else(|| anyhow::anyhow!("Python report escapes its module"))
    }
}

fn overlap(a: &str, b: &str) -> bool {
    a == b
        || a.strip_prefix(b).is_some_and(|rest| rest.starts_with('/'))
        || b.strip_prefix(a).is_some_and(|rest| rest.starts_with('/'))
}
