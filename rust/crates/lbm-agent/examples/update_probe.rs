//! The update check's Windows chain, short of running anything: what GitHub lists, the
//! release that would be offered with and without betas, its installer downloaded and
//! its signature checked; and the signature verdict on any file given.
//!
//! ```text
//! update_probe                 the list, the choice, the download, the verdict
//! update_probe <file>...       the verdict on each file
//! ```
//!
//! Nothing is installed or launched. A refused download is left in place to look at.

#[cfg(windows)]
fn main() {
    use lbm_agent::update;

    let files: Vec<String> = std::env::args().skip(1).collect();
    if !files.is_empty() {
        for file in files {
            let verdict = update::verify(std::path::Path::new(&file));
            println!("{file}: {verdict:?}");
        }
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async {
        println!("this build: {:?}", update::current());
        let platform = update::Windows::new().expect("an HTTP client");
        let releases = match update::releases(&platform).await {
            Ok(releases) => releases,
            Err(why) => {
                println!("no list: {why}");
                return;
            }
        };
        println!(
            "listed: {}",
            releases.as_array().map_or(0, |list| list.len())
        );
        let stable = update::newest(&releases, false);
        let beta = update::newest(&releases, true);
        println!("newest stable: {stable:?}");
        println!("newest with betas: {beta:?}");
        let Some(release) = stable else { return };
        match platform.download(&release).await {
            Ok(path) => {
                println!("downloaded: {}", path.display());
                println!("verdict: {:?}", update::verify(&path));
            }
            Err(why) => println!("download: {why}"),
        }
    });
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the update check is Windows only");
}
