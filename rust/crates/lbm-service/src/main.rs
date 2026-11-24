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
        // An administrator's, elevated: approving and withdrawing masters.
        Some("trust") => command(|| {
            let mut rest = std::env::args().skip(2);
            let id = rest.next().ok_or("trust <machine id> [<name>]")?;
            let name = rest.collect::<Vec<_>>().join(" ");
            lbm_service::win::commands::trust(&id, &name)
        }),
        Some("untrust") => command(|| {
            let id = std::env::args().nth(2).ok_or("untrust <machine id>")?;
            lbm_service::win::commands::untrust(&id)
        }),
        Some("list") => command(lbm_service::win::commands::list),
        _ => {
            eprintln!(
                "lbm-service run                      started by the Service Control Manager\n\
                 lbm-service trust <id> [<name>]      approve a master (elevated)\n\
                 lbm-service untrust <id>             withdraw one (elevated)\n\
                 lbm-service list                     the approved masters"
            );
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
fn command(run: impl FnOnce() -> Result<(), String>) -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn run() -> std::process::ExitCode {
    eprintln!("lbm-service only runs on Windows");
    std::process::ExitCode::from(2)
}
