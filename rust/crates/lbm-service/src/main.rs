//! `lbm-service`: the Windows service of the multi-PC mode
//! (docs/multi-pc-service-windows.md). Installed and started only by the installer,
//! never by the agent or the UI.

fn main() -> std::process::ExitCode {
    run()
}

#[cfg(windows)]
fn run() -> std::process::ExitCode {
    match std::env::args().nth(1).as_deref() {
        // The command line the installer registers.
        Some("run") => match lbm_service::win::service::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("[lbm-service] {error}");
                std::process::ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("lbm-service is started by the Service Control Manager (`lbm-service run`)");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(not(windows))]
fn run() -> std::process::ExitCode {
    eprintln!("lbm-service only runs on Windows");
    std::process::ExitCode::from(2)
}
