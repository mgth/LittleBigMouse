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
    println!("cargo:rerun-if-changed=../../build-support/version.rs");

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

include!("../../build-support/version.rs");
