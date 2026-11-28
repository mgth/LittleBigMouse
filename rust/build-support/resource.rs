// The Windows VERSIONINFO resource of a program the setup ships, for the build
// scripts that stamp one (lbm-hook for the hook and the injector, lbm-agent,
// lbm-service). Pulled in with `include!` beside version.rs, which it reads the
// version from.
//
// rustc emits no version resource by default. Users see the gap in the file
// properties and in Task Manager, and SignPath constrains the product name and
// version of every file it signs (SIGNING.md), which a bare exe cannot match.

/// `name` is the shipped file's, without `.exe`.
fn version_resource(name: &str, description: &str) -> winresource::WindowsResource {
    let version = resolve_version();
    let (major, minor, patch, build) = numeric_version(&version);
    let packed = (u64::from(major) << 48)
        | (u64::from(minor) << 32)
        | (u64::from(patch) << 16)
        | u64::from(build);

    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Little Big Mouse");
    res.set("FileDescription", description);
    res.set("CompanyName", "Mathieu GRENET");
    res.set("LegalCopyright", "Copyright (C) Mathieu GRENET");
    res.set("OriginalFilename", &format!("{name}.exe"));
    res.set("InternalName", name);
    // Strings keep the full version ("5.6.1", or "5.7.0-beta.1"); the fixed-info
    // fields below are numeric-only and carry the same value with the
    // pre-release suffix dropped.
    res.set("FileVersion", &version);
    res.set("ProductVersion", &version);
    res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    res
}
