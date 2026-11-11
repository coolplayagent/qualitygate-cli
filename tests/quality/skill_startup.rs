//! Execute runtime selection and repository inspection as separate Skill steps.

#[cfg(any(target_os = "linux", windows))]
mod native {
    use std::{
        path::{Path, PathBuf},
        process::{Command, Output},
    };

    fn snippet(reference: &str, heading: &str, language: &str) -> String {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("skills/qualitygate-cli/references/{reference}.md")),
        )
        .unwrap();
        code_block(&text, heading, language)
    }

    fn code_block(text: &str, heading: &str, language: &str) -> String {
        text.replace("\r\n", "\n")
            .split(heading)
            .nth(1)
            .unwrap()
            .split(&format!("```{language}\n"))
            .nth(1)
            .unwrap()
            .split("```")
            .next()
            .unwrap()
            .into()
    }

    #[test]
    fn skill_startup_extracts_lf_and_crlf_examples_identically() {
        for language in ["bash", "powershell"] {
            let text = format!("## Native shell\n\n```{language}\ncommand --version\n```\n");
            for text in [&text, &text.replace('\n', "\r\n")] {
                assert_eq!(
                    code_block(text, "## Native shell", language),
                    "command --version\n"
                );
            }
        }
    }

    fn copy_directory(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    struct Install {
        _temp: tempfile::TempDir,
        skill: PathBuf,
        repo: PathBuf,
        bin: PathBuf,
        path_bin: PathBuf,
    }

    impl Install {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let skill = temp.path().join("installed skill");
            let repo = temp.path().join("target repository");
            std::fs::create_dir_all(&repo).unwrap();
            std::fs::write(repo.join("policy checks.yaml"), "schema_version: 1\n").unwrap();
            let platform = if cfg!(windows) { "windows" } else { "linux" };
            let architecture = std::env::consts::ARCH;
            let binary = if cfg!(windows) {
                "qualitygate.exe"
            } else {
                "qualitygate"
            };
            let bin = skill
                .join(format!("assets/{platform}-{architecture}"))
                .join(binary);
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            let source = Path::new(env!("CARGO_BIN_EXE_qualitygate"));
            if std::fs::hard_link(source, &bin).is_err() {
                std::fs::copy(source, &bin).unwrap();
            }
            copy_directory(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("skills/qualitygate-cli/references/rules"),
                &skill.join("references/rules"),
            );
            let path_bin = temp.path().join("published bin");
            std::fs::create_dir_all(&path_bin).unwrap();
            std::fs::hard_link(&bin, path_bin.join(binary)).unwrap();
            Self {
                _temp: temp,
                skill,
                repo,
                bin,
                path_bin,
            }
        }

        fn run(&self, prefix: &str, override_path: Option<&Path>) -> Output {
            self.run_steps(prefix, override_path, None, true)
        }

        fn run_steps(
            &self,
            prefix: &str,
            override_path: Option<&Path>,
            runtime: Option<&Path>,
            inspect_configuration: bool,
        ) -> Output {
            let language = if cfg!(windows) { "powershell" } else { "bash" };
            let inspection = if inspect_configuration {
                snippet("verification", "## Inspect configuration", language)
            } else {
                String::new()
            };
            #[cfg(target_os = "linux")]
            let mut command = {
                let mut command = Command::new("bash");
                command.args([
                    "--noprofile",
                    "--norc",
                    "-c",
                    &format!(
                        "{prefix}\n{}\n{inspection}",
                        snippet("runtime-startup", "## Linux and WSL: Bash", "bash")
                    ),
                ]);
                command
            };
            #[cfg(windows)]
            let mut command = {
                let mut command = Command::new("pwsh");
                let script = format!(
                    "$QualitygateSkillRoot = $env:QUALITYGATE_SKILL_ROOT\n\
                     $QualitygateBin = $env:QUALITYGATE_BIN\n\
                     $RepositoryRoot = $env:REPOSITORY_ROOT\n\
                     $QualitygateConfig = $env:QUALITYGATE_CONFIG\n\
                     {prefix}\n{}\n{inspection}\nexit $LASTEXITCODE",
                    snippet("runtime-startup", "## Windows: PowerShell", "powershell")
                );
                command.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
                command
            };
            let mut paths = vec![self.path_bin.clone()];
            paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
            command
                .env("PATH", std::env::join_paths(paths).unwrap())
                .env("QUALITYGATE_SKILL_ROOT", &self.skill)
                .env_remove("REPOSITORY_ROOT")
                .env_remove("QUALITYGATE_CONFIG")
                .env_remove("QUALITYGATE_BIN")
                .env_remove("QUALITYGATE_BUILTIN_RULES_DIR")
                .current_dir(&self.path_bin);
            if inspect_configuration {
                command
                    .env("REPOSITORY_ROOT", &self.repo)
                    .env("QUALITYGATE_CONFIG", "policy checks.yaml");
            }
            if let Some(runtime) = runtime {
                command.env("QUALITYGATE_BIN", runtime);
            }
            if let Some(path) = override_path {
                command.env("QUALITYGATE_BUILTIN_RULES_DIR", path);
            }
            command.output().unwrap()
        }

        fn assert_configuration(&self, output: Output) {
            let values = Self::reports(output);
            assert_eq!(values.len(), 2, "capabilities then configuration");
            assert_eq!(values[1]["source"], "policy checks.yaml");
            assert!(!self.repo.join("qualitygate.yaml").exists());
            assert_eq!(
                std::fs::read_to_string(self.repo.join("policy checks.yaml")).unwrap(),
                "schema_version: 1\n"
            );
        }

        fn reports(output: Output) -> Vec<serde_json::Value> {
            assert!(
                output.status.success(),
                "stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            let json = &text[text.find('{').unwrap()..];
            serde_json::Deserializer::from_str(json)
                .into_iter()
                .collect::<Result<_, _>>()
                .unwrap()
        }
    }

    #[test]
    fn skill_runtime_only_needs_no_repository_or_configuration_and_never_initializes() {
        let install = Install::new();
        std::fs::remove_dir_all(&install.repo).unwrap();
        let values = Install::reports(install.run_steps("", None, None, false));
        assert_eq!(values.len(), 1, "runtime selection emits only capabilities");
        assert!(values[0]["capabilities"].is_array());
        assert_eq!(
            values[0]["limits"]["snapshot_default_file_bytes"],
            2 * 1024 * 1024
        );
        assert_eq!(
            values[0]["limits"]["snapshot_max_file_bytes"],
            8 * 1024 * 1024
        );
        assert!(!install.repo.exists());
        assert!(!install.path_bin.join("qualitygate.yaml").exists());
    }

    #[test]
    fn skill_explicit_runtime_is_retained_and_invalid_selection_does_not_fall_back() {
        let install = Install::new();
        let binary = install.bin.file_name().unwrap();
        let directory = install.skill.join("explicit runtime");
        std::fs::create_dir(&directory).unwrap();
        let selected = directory.join(binary);
        std::fs::hard_link(&install.bin, &selected).unwrap();
        // Both bundled and PATH binaries remain usable; neither may override this choice.
        let values = Install::reports(install.run_steps("", None, Some(&selected), true));
        assert_eq!(values.len(), 2);
        assert_eq!(
            Path::new(values[0]["executable"]["path"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            selected.canonicalize().unwrap()
        );
        assert_eq!(values[1]["source"], "policy checks.yaml");
        let missing = install.skill.join("missing runtime");
        assert!(
            !install
                .run_steps("", None, Some(&missing), false)
                .status
                .success()
        );
        let empty_selection = if cfg!(windows) {
            "$QualitygateBin = ''"
        } else {
            "QUALITYGATE_BIN=''"
        };
        assert!(
            !install
                .run_steps(empty_selection, None, None, false)
                .status
                .success()
        );
    }

    #[test]
    fn skill_startup_preserves_paths_overrides_and_configuration_failures() {
        let install = Install::new();
        install.assert_configuration(install.run("", None));
        // A lightweight Skill uses PATH and supplies matching external assets.
        std::fs::remove_file(&install.bin).unwrap();
        install.assert_configuration(install.run("", None));
        // An explicit valid override survives even if the default assets disappear.
        let override_path = install.skill.join("reviewed rule assets");
        std::fs::rename(install.skill.join("references/rules"), &override_path).unwrap();
        install.assert_configuration(install.run("", Some(&override_path)));
        let missing = install.skill.join("missing assets");
        assert!(!install.run("", Some(&missing)).status.success());
        std::fs::remove_file(install.repo.join("policy checks.yaml")).unwrap();
        assert!(!install.run("", Some(&override_path)).status.success());
        assert!(!install.repo.join("policy checks.yaml").exists());
        assert!(!install.repo.join("qualitygate.yaml").exists());
        std::fs::write(install.repo.join("policy checks.yaml"), "invalid: [\n").unwrap();
        assert!(!install.run("", Some(&override_path)).status.success());
        assert_eq!(
            std::fs::read_to_string(install.repo.join("policy checks.yaml")).unwrap(),
            "invalid: [\n"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn skill_startup_does_not_choose_linux_assets_on_another_os() {
        let install = Install::new();
        // Leave a present but invalid Linux asset; selecting it would emit an error.
        std::fs::remove_file(&install.bin).unwrap();
        std::fs::write(&install.bin, "not executable code\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&install.bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let output = install.run(
            "uname() { if [ \"$1\" = -s ]; then echo Darwin; else echo x86_64; fi; }",
            None,
        );
        assert!(output.stderr.is_empty(), "must not try the Linux asset");
        install.assert_configuration(output);
    }
}
