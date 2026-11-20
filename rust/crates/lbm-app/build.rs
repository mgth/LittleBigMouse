// Give lbm-app.exe the Little Big Mouse icon and a VERSIONINFO resource.
//
// rustc embeds no icon, so Explorer, shortcuts and pinned taskbar entries showed
// the generic executable icon. The picture is the C# window's ApplicationIcon
// (Assets/lbm-logo.ico), the same one the window itself is given in main.rs.
//
// Everything here is a no-op off Windows: the Linux build must stay unaffected.

use std::{env, fs, path::PathBuf};

const ICON: &str = "../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm-logo.ico";

fn main() {
    println!("cargo:rerun-if-env-changed=LBM_VERSION");
    println!("cargo:rerun-if-changed=../../build-support/version.rs");
    println!("cargo:rerun-if-changed={ICON}");

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
    // The first icon group is the one Windows shows for the file.
    res.set_icon(ICON);
    res.set("ProductName", "Little Big Mouse");
    res.set("FileDescription", "Little Big Mouse");
    res.set("CompanyName", "Mathieu GRENET");
    res.set("LegalCopyright", "Copyright (C) Mathieu GRENET");
    res.set("OriginalFilename", "lbm-app.exe");
    res.set("InternalName", "lbm-app");
    res.set("FileVersion", &version);
    res.set("ProductVersion", &version);
    res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);

    if let Err(e) = res.compile() {
        // A missing icon must not cost the Windows build its window; CI asserts on
        // the resource separately.
        println!("cargo:warning=could not embed the icon and version resource: {e}");
    }
}

include!("../../build-support/version.rs");
