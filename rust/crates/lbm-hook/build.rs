// Stamp a Windows VERSIONINFO resource onto the hook daemon and the injector.
//
// One resource per binary: a single one for the package would be linked into both,
// and the injector would call itself the hook.
//
// Everything here is a no-op off Windows: the Linux build must stay unaffected.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=LBM_VERSION");
    println!("cargo:rerun-if-changed=../../build-support/version.rs");
    println!("cargo:rerun-if-changed=../../build-support/resource.rs");

    // Cargo builds this crate for Linux too; only a Windows target gets a resource.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // The cargo target, then the shipped name: the hook is built as lbm-hook.exe
    // and renamed during staging (see the [[bin]] note in Cargo.toml).
    let binaries = [
        (
            "lbm-hook",
            "LittleBigMouse.Hook",
            "Little Big Mouse hook daemon",
        ),
        (
            "lbm-inject",
            "lbm-inject",
            "Little Big Mouse input injector",
        ),
    ];
    for (target, name, description) in binaries {
        if let Err(e) = version_resource(name, description).compile_for(target) {
            // Failing the build here would take the Windows hook down over
            // metadata. Warn loudly instead; CI asserts on the resource separately.
            println!("cargo:warning=could not embed the version resource of {target}: {e}");
        }
    }
}

include!("../../build-support/version.rs");
include!("../../build-support/resource.rs");
