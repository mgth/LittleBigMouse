// Stamp a Windows VERSIONINFO resource onto lbm-service.exe.
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

    // Only the program: the package's examples and tests are not shipped.
    if let Err(e) =
        version_resource("lbm-service", "Little Big Mouse input service").compile_for("lbm-service")
    {
        // A missing resource must not cost the Windows build its service; CI asserts on
        // the resource separately.
        println!("cargo:warning=could not embed the version resource: {e}");
    }
}

include!("../../build-support/version.rs");
include!("../../build-support/resource.rs");
