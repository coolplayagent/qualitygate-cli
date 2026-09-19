use std::path::Path;

#[test]
fn pages_site_links_resolve_and_match_the_release_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let site = root.join("site");
    let html = std::fs::read_to_string(site.join("index.html")).unwrap();
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(site.join(".nojekyll").is_file());
    assert!(html.contains(&format!("releases/tag/{tag}")));
    assert!(html.contains(&format!("qualitygate-cli-skill-{tag}.tar.gz")));
    assert!(html.contains("--strip-components=1"));
    for documentation in [
        "docs/en/README.md",
        "docs/zh/README.md",
        "docs/en/01-user-guide/01-installation-and-first-check.md",
    ] {
        assert!(
            html.contains(&format!(
                "https://github.com/coolplayagent/qualitygate-cli/blob/main/{documentation}"
            )),
            "missing documentation entry {documentation}"
        );
    }

    for item in html.split("href=\"").skip(1) {
        let href = item.split('"').next().unwrap();
        if let Some(anchor) = href.strip_prefix('#') {
            assert!(
                html.contains(&format!("id=\"{anchor}\"")),
                "missing anchor {href}"
            );
        } else if let Some(path) =
            href.strip_prefix("https://github.com/coolplayagent/qualitygate-cli/blob/main/")
        {
            assert!(root.join(path).is_file(), "missing linked source {href}");
        } else if href.starts_with("https://github.com/coolplayagent/qualitygate-cli") {
            assert!(!href.contains(".."), "unsafe external link {href}");
        } else {
            assert!(
                !href.starts_with('/') && !href.contains(".."),
                "unsafe local link {href}"
            );
            assert!(site.join(href).exists(), "missing site asset {href}");
        }
    }
}

#[test]
fn pages_workflow_deploys_only_the_static_site_from_main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = std::fs::read_to_string(root.join(".github/workflows/pages.yml")).unwrap();
    let parsed: serde_norway::Value = serde_norway::from_str(&workflow).unwrap();
    assert_eq!(parsed["on"]["push"]["branches"][0], "main");
    assert_eq!(
        parsed["jobs"]["deploy"]["environment"]["name"],
        "github-pages"
    );
    assert_eq!(parsed["permissions"]["pages"], "write");
    assert_eq!(parsed["permissions"]["id-token"], "write");
    assert!(workflow.contains("actions/configure-pages@v5"));
    assert!(workflow.contains("actions/upload-pages-artifact@v4"));
    assert!(workflow.contains("actions/deploy-pages@v4"));
    assert!(workflow.contains("path: site"));
}
