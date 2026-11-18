//! Project-owned CMake/CTest driver. Qualitygate owns the outer command deadline.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    objects: Vec<PathBuf>,
    // Expected CTest names and their native executable paths in the build tree.
    tests: BTreeMap<String, PathBuf>,
}

fn read(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(8_388_609)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 8_388_608,
        "File exceeds 8 MiB: {}",
        path.display()
    );
    Ok(String::from_utf8(bytes)?)
}

fn launcher(mode: &str, args: &[String]) -> Result<()> {
    let (compiler, flags) = args.split_first().context("Missing compiler")?;
    let output = flags
        .windows(2)
        .find(|pair| pair[0] == "-o")
        .context("Missing compiler output")?;
    let mut cmd =
        if mode == "compile" && env::var_os("ASAN_USE_CCACHE").as_deref() == Some("1".as_ref()) {
            let mut cmd = Command::new("ccache");
            cmd.arg(compiler);
            cmd
        } else {
            Command::new(compiler)
        };
    if mode == "compile" {
        cmd.arg(format!(
            "-fdebug-prefix-map={}=.",
            env::current_dir()?.display()
        ));
    }
    ensure!(
        cmd.args(flags)
            .args([
                "-fsanitize=address",
                "-fno-sanitize-recover=address",
                "-fno-omit-frame-pointer"
            ])
            .status()?
            .success(),
        "{mode} failed"
    );
    fs::write(format!("{}.asan-{mode}", output[1]), b"ok")?;
    Ok(())
}

fn artifact(build: &Path, relative: &Path, mode: &str) -> Result<PathBuf> {
    let path = build.join(relative).canonicalize()?;
    ensure!(
        path.starts_with(build) && path.is_file(),
        "Unexpected build artifact"
    );
    ensure!(
        fs::read(format!("{}.asan-{mode}", path.display()))? == b"ok",
        "Missing instrumentation receipt"
    );
    Ok(path)
}

fn check(name: &str) -> Result<u8> {
    let suites: BTreeMap<String, Suite> =
        serde_json::from_str(&read(Path::new("tools/asan/suites.json"))?)?;
    let suite = suites.get(name).context("Unknown suite")?;
    ensure!(
        !suite.objects.is_empty() && !suite.tests.is_empty(),
        "Empty suite inventory"
    );
    let tmp = tempfile::tempdir_in(".")?;
    let build = tmp.path().canonicalize()?;
    let driver = env::current_exe()?;
    let mut cmake = Command::new("cmake");
    cmake.args(["-S", ".", "-B"]).arg(&build).args([
        "-G",
        "Ninja",
        "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
        "-DCMAKE_C_COMPILER=clang",
        "-DCMAKE_CXX_COMPILER=clang++",
    ]);
    for lang in ["C", "CXX"] {
        for (role, mode) in [("COMPILER", "compile"), ("LINKER", "link")] {
            cmake.arg(format!(
                "-DCMAKE_{lang}_{role}_LAUNCHER={};{mode}",
                driver.display()
            ));
        }
    }
    ensure!(
        cmake
            .stdout(Stdio::from(std::io::stderr()))
            .status()?
            .success(),
        "CMake configure failed"
    );
    ensure!(
        Command::new("cmake")
            .env("CCACHE_BASEDIR", env::current_dir()?)
            .arg("--build")
            .arg(&build)
            .stdout(Stdio::from(std::io::stderr()))
            .status()?
            .success(),
        "CMake build failed"
    );
    for object in &suite.objects {
        artifact(&build, object, "compile")?;
    }
    let inventory = build.join("inventory.json");
    let status = Command::new("ctest")
        .arg("--test-dir")
        .arg(&build)
        .arg("--show-only=json-v1")
        .stdout(fs::File::create(&inventory)?)
        .status()?;
    ensure!(status.success(), "CTest discovery failed");
    let data: Value = serde_json::from_str(&read(&inventory)?)?;
    let mut selected = String::new();
    for (test, binary) in &suite.tests {
        ensure!(
            !test.contains(['\r', '\n']) && !test.trim().is_empty(),
            "Invalid test name"
        );
        let exe = artifact(&build, binary, "link")?;
        let mut matches = data["tests"]
            .as_array()
            .context("Missing CTest inventory")?
            .iter()
            .filter(|row| row["name"].as_str() == Some(test));
        let row = matches.next().context("Expected test missing")?;
        ensure!(
            matches.next().is_none()
                && row["command"][0].as_str().map(Path::new) == Some(exe.as_path()),
            "Unexpected test executable: {test}"
        );
        selected.push_str(test);
        selected.push('\n');
    }
    let list = build.join("selected.txt");
    fs::write(&list, selected)?;
    let report = build.join("results.xml");
    let status = Command::new("ctest")
        .arg("--test-dir")
        .arg(&build)
        .arg("--output-on-failure")
        .arg("--tests-from-file")
        .arg(list)
        .args([
            "--no-tests=error",
            "--test-output-size-failed",
            "1048576",
            "--output-junit",
        ])
        .arg(&report)
        .env("ASAN_OPTIONS", "halt_on_error=1:detect_leaks=1")
        .stdout(Stdio::from(std::io::stderr()))
        .status()?;
    summarize(&read(&report)?, suite, status.code(), &build)
}

fn source_location(log: &str, root: &Path, build: &Path) -> Option<(String, usize)> {
    // Clang may record source paths relative to Ninja's compilation directory.
    log.lines()
        .filter(|line| line.trim_start().starts_with('#'))
        .find_map(|frame| {
            let location = frame.split_whitespace().last()?;
            let (file, position) = location.split_once(':')?;
            let line = position.split(':').next()?.parse::<usize>().ok()?;
            let path = build.join(file).canonicalize().ok()?;
            let relative = path.strip_prefix(root).ok()?;
            (line > 0 && path.is_file() && !path.starts_with(build))
                .then_some((relative.to_str()?.to_owned(), line))
        })
}

fn summarize(xml: &str, suite: &Suite, status: Option<i32>, build: &Path) -> Result<u8> {
    let doc = roxmltree::Document::parse(xml)?;
    ensure!(
        doc.root_element().has_tag_name("testsuite"),
        "Expected a CTest testsuite"
    );
    let mut seen = BTreeSet::new();
    let mut issues = Vec::new();
    for case in doc.descendants().filter(|n| n.has_tag_name("testcase")) {
        let name = case.attribute("name").context("Missing test name")?;
        ensure!(
            suite.tests.contains_key(name) && seen.insert(name),
            "Unexpected or repeated test: {name}"
        );
        ensure!(
            matches!(case.attribute("status"), Some("run" | "fail"))
                && !case
                    .children()
                    .any(|n| n.has_tag_name("skipped") || n.has_tag_name("error")),
            "Test did not execute normally: {name} ({:?})",
            case.attribute("status")
        );
        let log = case
            .children()
            .find(|n| n.has_tag_name("system-out"))
            .and_then(|n| n.text())
            .unwrap_or("");
        let finding = log.lines().find(|s| {
            s.contains("ERROR: AddressSanitizer:") || s.contains("ERROR: LeakSanitizer:")
        });
        ensure!(
            case.attribute("status") != Some("fail")
                || case.children().any(|n| n.has_tag_name("failure")),
            "Missing failure outcome: {name}"
        );
        if let Some(detail) = finding {
            eprintln!("{name}: {}", log.chars().take(8192).collect::<String>());
            let kind = detail
                .split_once("ERROR: ")
                .context("Missing ASan detail")?
                .1;
            let mut issue = json!({"rule":"asan","message":format!("{name}: {kind}")});
            if let Some((file, line)) = source_location(log, &env::current_dir()?, build) {
                issue["file"] = json!(file);
                issue["line"] = json!(line);
            }
            issues.push(issue);
        } else if let Some(failure) = case.children().find(|n| n.has_tag_name("failure")) {
            ensure!(
                failure.attribute("message") == Some("Failed")
                    && log.trim().len() < 1024 * 1024
                    && !log.contains("AddressSanitizer"),
                "Unclassified failure: {name}"
            );
            issues.push(json!({"rule":"test-failure","message":format!("{name} failed")}));
        }
    }
    ensure!(seen.len() == suite.tests.len(), "Incomplete test inventory");
    ensure!(
        status == Some(0) || status == Some(8) && !issues.is_empty(),
        "Unexplained CTest exit: {status:?}"
    );
    let code = if issues.is_empty() { 0 } else { 10 };
    println!(
        "{}",
        json!({"issues":issues,"tests":{"executed":seen.len(),"failures":issues.len(),"skipped":0}})
    );
    Ok(code)
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    let result = match args.as_slice() {
        [version] if version == "--version" => {
            println!("asan-check 0.1.0");
            Ok(0)
        }
        [arg, name] if arg == "--suite" => check(name),
        [mode, rest @ ..] if matches!(mode.as_str(), "compile" | "link") => {
            launcher(mode, rest).map(|()| 0)
        }
        _ => Err(anyhow::anyhow!("Expected --suite NAME or --version")),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(case: &str, exit: i32) -> Result<u8> {
        let suite = Suite {
            objects: vec!["native.o".into()],
            tests: BTreeMap::from([("memory".into(), "native".into())]),
        };
        summarize(
            &format!("<testsuite>{case}</testsuite>"),
            &suite,
            Some(exit),
            Path::new("."),
        )
    }

    #[test]
    fn completed_runs_distinguish_asan_and_test_failures_from_missing_execution() {
        assert_eq!(
            run(r#"<testcase status="run" name="memory"/>"#, 0).unwrap(),
            0
        );
        for output in [
            "ERROR: AddressSanitizer: heap-use-after-free",
            "assertion failed",
        ] {
            let failed = format!(
                r#"<testcase name="memory" status="fail"><failure message="Failed"/><system-out>{output}</system-out></testcase>"#
            );
            assert_eq!(run(&failed, 8).unwrap(), 10);
        }
        for case in [
            "",
            r#"<testcase status="run" name="other"/>"#,
            r#"<testcase status="notrun" name="memory"><skipped/></testcase>"#,
            r#"<testcase status="fail" name="memory"><failure message="Timeout"/></testcase>"#,
        ] {
            assert!(run(case, 8).is_err());
        }
        assert!(run(r#"<testcase status="run" name="memory"/>"#, 8).is_err());
    }
}
