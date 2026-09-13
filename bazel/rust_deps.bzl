"""Cargo-root dependency adapters for Bazel source packages."""

load(
    "@crates//:defs.bzl",
    _aliases = "aliases",
    _all_crate_deps = "all_crate_deps",
    _crate_edition = "crate_edition",
)

_CARGO_ROOT_PACKAGE = ""

def aliases(
        normal = False,
        normal_dev = False,
        proc_macro = False,
        proc_macro_dev = False,
        build = False,
        build_proc_macro = False):
    return _aliases(
        normal = normal,
        normal_dev = normal_dev,
        proc_macro = proc_macro,
        proc_macro_dev = proc_macro_dev,
        build = build,
        build_proc_macro = build_proc_macro,
        package_name = _CARGO_ROOT_PACKAGE,
    )

def all_crate_deps(
        normal = False,
        normal_dev = False,
        proc_macro = False,
        proc_macro_dev = False,
        build = False,
        build_proc_macro = False):
    return _all_crate_deps(
        normal = normal,
        normal_dev = normal_dev,
        proc_macro = proc_macro,
        proc_macro_dev = proc_macro_dev,
        build = build,
        build_proc_macro = build_proc_macro,
        package_name = _CARGO_ROOT_PACKAGE,
    )

def crate_edition():
    return _crate_edition(package_name = _CARGO_ROOT_PACKAGE)
