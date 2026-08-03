// Stamp a Windows VERSIONINFO resource onto the hook daemon.
//
// rustc emits none by default, so LittleBigMouse.Hook.exe has been shipping with
// no product name, no version and no copyright at all, sitting next to a UI exe
// that carries all three. Users see the gap in the file properties and in Task
// Manager, and a code-signing artifact configuration that constrains product and
// version metadata (SignPath does) has nothing to match against.
//
// Everything here is a no-op off Windows: the Linux build must stay unaffected.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=LBM_VERSION");

    // Cargo builds this crate for Linux too; only a Windows target gets a resource.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = resolve_version();
    let (major, minor, patch, build) = numeric_version(&version);
    let packed = (u64::from(major) << 48)
        | (u64::from(minor) << 32)
        | (u64::from(patch) << 16)
        | u64::from(build);

    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Little Big Mouse");
    res.set("FileDescription", "Little Big Mouse hook daemon");
    res.set("CompanyName", "Mathieu GRENET");
    res.set("LegalCopyright", "Copyright (C) Mathieu GRENET");
    // The shipped name, not the cargo target name: the binary is built as
    // lbm-hook.exe and renamed during staging (see the [[bin]] note in Cargo.toml).
    res.set("OriginalFilename", "LittleBigMouse.Hook.exe");
    res.set("InternalName", "LittleBigMouse.Hook");
    // Strings keep the full version ("5.6.1", or "5.7.0-beta.1"); the fixed-info
    // fields below are numeric-only and carry the same value with the
    // pre-release suffix dropped.
    res.set("FileVersion", &version);
    res.set("ProductVersion", &version);
    res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);

    if let Err(e) = res.compile() {
        // Failing the build here would take the Windows hook down over metadata.
        // Warn loudly instead; CI asserts on the resource separately.
        println!("cargo:warning=could not embed version resource: {e}");
    }
}

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

    let props = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("..")
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
