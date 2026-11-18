//! Run explicitly with the reference C/C++ toolchain on PATH.
use serde_json::{Value, json};
use std::{
    fs,
    process::{Command, Output},
};

#[test]
#[ignore = "requires Linux, CMake/CTest >= 3.29, Ninja, Clang and the ASan runtime"]
fn native_suite_tracks_actual_compilation_execution_and_failures() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
    fs::create_dir_all(root.join("tools/asan")).unwrap();
    fs::write(root.join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.29)\nproject(sample LANGUAGES CXX)\nenable_testing()\nadd_executable(native owner.cpp reader.cpp)\nadd_test(NAME memory COMMAND native)\n").unwrap();
    fs::write(
        root.join("tools/asan/suites.json"),
        include_str!("../suites.json"),
    )
    .unwrap();
    fs::write(
        root.join("reader.cpp"),
        "int read_value(int *value) {return *value;}",
    )
    .unwrap();
    let run = |source: &str| -> Output {
        fs::write(root.join("owner.cpp"), source).unwrap();
        Command::new(env!("CARGO_BIN_EXE_asan-check"))
            .current_dir(root)
            .args(["--suite", "native"])
            .output()
            .unwrap()
    };
    let clean = "int read_value(int*); int main(){int *p=new int(0); int r=read_value(p); delete p; return r;}";
    for (source, code, rule) in [
        (clean, 0, None),
        (
            "int read_value(int*); int main(){int *p=new int(0); delete p; return read_value(p);}",
            10,
            Some("asan"),
        ),
        ("int main(){return 1;}", 10, Some("test-failure")),
    ] {
        let output = run(source);
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["tests"]["executed"], json!(1));
        assert_eq!(report["issues"][0]["rule"].as_str(), rule);
    }
    fs::write(
        root.join("tools/asan/suites.json"),
        r#"{"native":{"objects":["unverified.o"],"tests":{"memory":"native"}}}"#,
    )
    .unwrap();
    assert_eq!(run(clean).status.code(), Some(2));
}
