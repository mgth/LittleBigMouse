//! Native pattern viewer; also embedded in lbm-app as --test-pattern-host.
#[cfg(target_os = "linux")]
fn main() {
    std::process::exit(lbm_pattern::viewer::run(std::env::args().skip(1)));
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("lbm-pattern requires Linux/Wayland");
    std::process::exit(2);
}
