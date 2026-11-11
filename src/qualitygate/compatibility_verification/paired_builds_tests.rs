use super::*;
use serde_json::json;
use std::io::{Cursor, Write};

fn jar(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap().into_inner()
}

#[cfg(unix)]
fn fixture(root: &Path, external: &Path, incompatible: bool) -> (CommandCheck, Arc<Snapshot>) {
    use std::os::unix::fs::PermissionsExt;
    let analyzer = jar(
        "META-INF/maven/com.github.siom79.japicmp/japicmp/pom.properties",
        b"groupId=com.github.siom79.japicmp\nartifactId=japicmp\nversion=0.26.2\n",
    );
    std::fs::write(external.join("analyzer.jar"), &analyzer).unwrap();
    // Fixed protocol double: validates orchestration/archive binding without a JVM dependency.
    let tool = external.join("java-fixture");
    let script = r#"#!/bin/sh
if [ "$1" = '-version' ]; then printf 'fixture-java-1'; exit 0; fi
while [ "$#" -gt 0 ]; do
 case "$1" in
 --old) shift; old="$1";;
 --new) shift; new="$1";;
 --xml-file) shift; output="$1";;
 -a) shift; access="$1";;
 esac
 shift
done
if [ "$access" = private ]; then access=PRIVATE; else access=PROTECTED; fi
printf '<japicmp oldJar="%s" newJar="%s" accessModifier="%s" packagesInclude="all" packagesExclude="n.a." ignoreMissingClasses="false" ignoreMissingClassesByRegularExpressions="" onlyModifications="false" onlyBinaryIncompatibleModifications="false"><classes><class fullyQualifiedName="Api" binaryCompatible="COMPATIBLE" sourceCompatible="COMPATIBLE"><modifiers><modifier oldValue="PUBLIC" newValue="PUBLIC"/></modifiers></class></classes></japicmp>' "$old" "$new" "$access" > "$output"
"#.replace("COMPATIBLE", if incompatible { "false" } else { "true" });
    std::fs::write(&tool, script).unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    let check = serde_json::from_value(json!({"id":"api","kind":"command","argv":["sh","-c","cp input.jar output.jar; cp dependency.jar dependency-output.jar"],
        "timeout_seconds":2,"tools":[{"id":"builder","argv":["sh","-c","printf build-fixture-v1"]}],
        "compatibility":{"tool":"japicmp","analyzer_jar":external.join("analyzer.jar"),"analyzer_sha256":snapshot::digest(&analyzer),"java":tool,
        "timeout_seconds":1,"artifacts":[{"baseline":"output.jar","current":"output.jar"}],
        "classpath":[{"baseline":"dependency-output.jar","current":"dependency-output.jar"}]}})).unwrap();
    let mut snapshot = crate::application::test_support::snapshot(root);
    for (name, class) in [
        ("input.jar", "Api.class"),
        ("dependency.jar", "Helper.class"),
    ] {
        snapshot.files.insert(
            name.into(),
            File {
                bytes: jar(class, b"inventory fixture"),
                executable: false,
            },
        );
    }
    snapshot.base_files = snapshot.files.clone();
    (check, Arc::new(snapshot))
}

#[cfg(unix)]
#[tokio::test]
async fn paired_builds_keep_archives_bound_and_compare_inventory_before_public_api() {
    for incompatible in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let artifacts = tempfile::tempdir().unwrap();
        let (check, snapshot) = fixture(root.path(), external.path(), incompatible);
        let result = execute(&check, artifacts.path(), &snapshot).await;
        assert_eq!(
            result.execution.status,
            ExecutionStatus::Completed,
            "{:?}",
            result.execution
        );
        assert_eq!(
            result.verdict,
            Some(if incompatible {
                Verdict::Fail
            } else {
                Verdict::Pass
            })
        );
        assert_eq!(result.metadata["compatibility"]["inventory_classes"], 1);
        assert_eq!(
            result.metadata["compatibility"]["archives"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            result.metadata["compatibility"]["comparison"]["binary_compatible"],
            !incompatible
        );
        if incompatible {
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("CLASS_INCOMPATIBLE"))
            );
        }
        assert!(
            result
                .execution
                .artifacts
                .iter()
                .any(|a| a.path.ends_with("compatibility-api.xml"))
        );
        assert!(
            result
                .execution
                .artifacts
                .iter()
                .any(|a| a.path.ends_with("compatibility-inventory.xml"))
        );
        assert!(root.path().read_dir().unwrap().next().is_none());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn analyzer_identity_build_failure_and_timeout_never_report_compatibility_success() {
    for case in 0..5 {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let artifacts = tempfile::tempdir().unwrap();
        let (mut check, snapshot) = fixture(root.path(), external.path(), false);
        match case {
            0 => check.compatibility = None,
            1 => {
                check.compatibility.as_mut().unwrap().analyzer_sha256 =
                    snapshot::digest(b"wrong analyzer")
            }
            2 => check.argv[2] = "exit 7".into(),
            3 => {
                check.argv[2] = "sleep 4".into();
                check.timeout_seconds = 1;
            }
            4 => std::fs::write(
                external.path().join("java-fixture"),
                "#!/bin/sh\nif [ \"$1\" = -version ]; then printf fixture; else exit 7; fi\n",
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let result = execute(&check, artifacts.path(), &snapshot).await;
        assert_ne!(result.verdict, Some(Verdict::Pass), "case {case}");
        assert_eq!(
            result.execution.status,
            if case == 3 {
                ExecutionStatus::TimedOut
            } else {
                ExecutionStatus::Blocked
            }
        );
        if case >= 2 {
            assert!(!result.execution.artifacts.is_empty());
        }
    }
}
