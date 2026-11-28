// Version resolution shared by the build scripts that stamp a Windows VERSIONINFO
// resource (lbm-hook, lbm-agent, lbm-service, lbm-app). Pulled in with `include!`, so each build script
// keeps its own dependencies (`roxmltree`) and nothing here becomes a crate.

/// LBM_VERSION (release CI, from the git tag) > Directory.Build.props (the
/// repo's single source of truth for the version) > the crate version.
///
/// Reading the props file is what keeps the hook from drifting: the .NET side
/// takes its version from there, and nothing would otherwise stop this crate
/// from shipping 0.1.0 inside a 5.6.0 installer.
fn resolve_version() -> String {
    if let Ok(v) = env::var("LBM_VERSION") {
        let v = v.trim();
        if !v.is_empty() {
            return v.to_owned();
        }
    }

    // rust/crates/<crate> -> repository root.
    let props = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("Directory.Build.props");
    println!("cargo:rerun-if-changed={}", props.display());

    if let Some(v) = fs::read_to_string(&props)
        .ok()
        .and_then(|s| version_element(&s))
    {
        return v;
    }

    // Standalone checkout of the crate (no props file next to it).
    env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned())
}

/// Text of the `<Version>` element in an MSBuild props file.
///
/// Parsed rather than substring-matched: Directory.Build.props documents itself
/// with a comment that spells out "<Version>" several lines above the real
/// element, and a `find("<Version>")` happily returns the comment instead.
fn version_element(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let text = doc
        .descendants()
        .find(|n| n.is_element() && n.has_tag_name("Version"))?
        .text()?
        .trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// "5.6.1-beta.2" -> (5, 6, 1, 0). VERSIONINFO's fixed part is four u16s, so the
/// pre-release suffix is dropped and missing components read as zero.
fn numeric_version(version: &str) -> (u16, u16, u16, u16) {
    let mut parts = version
        .split('-')
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.trim().parse::<u16>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
