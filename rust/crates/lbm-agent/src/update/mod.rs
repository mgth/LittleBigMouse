//! The update check — what C#'s `ApplicationUpdaterViewModel` did, Windows only.
//!
//! The GitHub releases carry the Windows installer and nothing else; everywhere else a
//! package manager owns updates, so the Linux agent has no check at all (`windows` is the
//! only platform, and the tray offers the entry only there).
//!
//! **When.** At startup — a minute in, once the agent knows the options — then once a
//! day while it runs, if the user left "Check for updates automatically" on; and whenever
//! the tray's "Check for update" is chosen. Nothing else: no timer runs faster than a
//! day, a check the option forbids is not made, and none is made automatically while the
//! tray icon is hidden (there would be nowhere to say what it found).
//!
//! **What is asked.** `GET` [`FIRST_PAGE`], the public GitHub Releases API, with a
//! `LittleBigMouse` user agent (PRIVACY.md), a hundred releases a page and every page
//! GitHub links as `next` ([`releases`]): a stable release behind a page of betas is
//! still found. The newest published release wins: drafts
//! are skipped, pre-releases are offered only under "Include beta versions", and a
//! release needs an installer (`.exe`) asset and a version, read from its name and else
//! from its tag, as C# did.
//!
//! **What is done.** The tray says a newer version exists. Clicking that notice
//! downloads the installer, checks its Authenticode signature (valid, and signed by
//! [`PUBLISHER`], the identity SIGNING.md has the release workflow insist on) and runs
//! it elevated — the file held open from the check to the launch, so that what runs is
//! what was checked. A file that fails the check is deleted unrun and the notice offers the
//! release page instead.
//!
//! The loop is [`run`], over a [`Platform`] so that it can be tested without a network
//! or a Windows desktop; the pure decisions ([`newest`], [`parse_version`]) are tested on
//! every platform.

use std::future::Future;
use std::time::Duration;

use semver::Version;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;

use crate::api::{self, Call, Request};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{verify, Windows};

/// The first page of the public list of releases, newest first: as many releases as GitHub gives at once.
pub const FIRST_PAGE: &str =
    "https://api.github.com/repos/mgth/LittleBigMouse/releases?per_page=100";
/// Where every page GitHub links to must be (it names the repository by number there).
const API: &str = "https://api.github.com/";
/// Pages followed at most: a thousand releases. A list longer than that is not believed
/// (it would only ever be a loop), and the check fails rather than say "up to date".
const MAX_PAGES: usize = 10;
/// Where a release's page and its files must be, compared without regard to case.
const RELEASE_PAGES: &str = "https://github.com/mgth/littlebigmouse/releases/";
/// The only publisher whose installer is run (SIGNING.md).
pub const PUBLISHER: &str = "SignPath Foundation";
/// From the agent's start to its first automatic check: the logon's network may not be
/// up yet, and the options are not known before the first state.
pub const FIRST_CHECK: Duration = Duration::from_secs(60);
/// Between two automatic checks.
pub const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The version this agent was released under (`LBM_VERSION`, else Directory.Build.props;
/// see build.rs). `None` would mean a version nobody can compare, and no check is made.
pub fn current() -> Option<Version> {
    parse_version(env!("LBM_RELEASE_VERSION"))
}

/// A release worth offering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The release's page on GitHub.
    pub page: String,
    /// The installer's download address.
    pub installer: String,
    /// The installer's file name, as GitHub has it (a plain name, checked).
    pub file_name: String,
    /// Its size in bytes, as GitHub has it: what the download must come to.
    pub size: u64,
}

/// "5.6.0", "v5.3.0", "5.2.3.0", "6.0.0-beta.1": a version, the fourth number of the
/// old four-part names dropped (it was always 0, and semver has three).
pub fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let (numbers, rest) = match text.find(['-', '+']) {
        Some(at) => text.split_at(at),
        None => (text, ""),
    };
    let parts: Vec<u64> = numbers
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [major, minor, patch] = match parts[..] {
        [major] => [major, 0, 0],
        [major, minor] => [major, minor, 0],
        [major, minor, patch] | [major, minor, patch, _] => [major, minor, patch],
        _ => return None,
    };
    Version::parse(&format!("{major}.{minor}.{patch}{rest}")).ok()
}

/// The newest release of the API's list that may be offered, betas only when
/// `include_beta`.
pub fn newest(releases: &Value, include_beta: bool) -> Option<Release> {
    releases
        .as_array()?
        .iter()
        .filter(|r| r["draft"] != true)
        .filter(|r| include_beta || r["prerelease"] != true)
        .filter_map(release)
        .filter(|r| include_beta || r.version.pre.is_empty())
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// One entry of the API's list, if it names a version and carries an installer.
fn release(entry: &Value) -> Option<Release> {
    let version = entry["name"]
        .as_str()
        .and_then(parse_version)
        .or_else(|| entry["tag_name"].as_str().and_then(parse_version))?;
    let page = entry["html_url"].as_str().filter(|u| ours(u))?.to_owned();
    let asset = entry["assets"].as_array()?.iter().find(|a| {
        a["name"].as_str().is_some_and(installer_name)
            && a["browser_download_url"].as_str().is_some_and(ours)
    })?;
    Some(Release {
        version,
        page,
        installer: asset["browser_download_url"].as_str()?.to_owned(),
        file_name: asset["name"].as_str()?.to_owned(),
        size: asset["size"].as_u64()?,
    })
}

/// An address on this project's release pages (or files: they live below them).
fn ours(url: &str) -> bool {
    url.get(..RELEASE_PAGES.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(RELEASE_PAGES))
}

/// A name the installer can be saved under as it is: an `.exe`, no path, nothing a
/// shell would read twice.
fn installer_name(name: &str) -> bool {
    name.len() > 4
        && name.len() <= 100
        && name.to_ascii_lowercase().ends_with(".exe")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !name.starts_with('.')
}

/// One page of the API's list, and the address of the next one, from the `Link` header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub releases: Value,
    pub next: Option<String>,
}

/// The address a `Link` header gives as `rel="next"`, if it is GitHub's API.
pub fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|link| {
        let (target, params) = link.trim().strip_prefix('<')?.split_once('>')?;
        let next = params.split(';').any(|param| {
            param.trim().strip_prefix("rel=").is_some_and(|rel| {
                rel.trim_matches('"')
                    .split_ascii_whitespace()
                    .any(|r| r.eq_ignore_ascii_case("next"))
            })
        });
        (next && on_api(target)).then(|| target.to_owned())
    })
}

fn on_api(url: &str) -> bool {
    url.get(..API.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(API))
}

/// The whole list, every page followed. Any page that fails fails the list: an answer
/// from part of it could call a user up to date who is not.
pub async fn releases(platform: &impl Platform) -> Result<Value, String> {
    let mut all = Vec::new();
    let mut url = FIRST_PAGE.to_owned();
    for _ in 0..MAX_PAGES {
        let page = platform.page(&url).await?;
        let Value::Array(list) = page.releases else {
            return Err("GitHub's answer is not a list of releases".to_owned());
        };
        all.extend(list);
        match page.next.filter(|next| on_api(next)) {
            Some(next) => url = next,
            None => return Ok(Value::Array(all)),
        }
    }
    Err(format!(
        "GitHub lists more than {MAX_PAGES} pages of releases"
    ))
}

/// What the tray is asked to do, from its menu or from a click on a notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Look now and say what was found, "up to date" included.
    Check,
    /// Download, check and run the installer of the release last found.
    Install,
    /// Open the page of the release last found.
    OpenPage,
}

/// A notice for the tray to show, and what a click on it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub text: String,
    /// A warning rather than news.
    pub warning: bool,
    pub click: Option<Command>,
}

/// What a platform does for the loop: Windows' network, signature check and shell.
pub trait Platform: Send + Sync {
    /// One page of the API's list of releases (the JSON array, and the next page's
    /// address).
    fn page(&self, url: &str) -> impl Future<Output = Result<Page, String>> + Send;
    /// Downloads, checks and runs the installer of `release`. The error says why
    /// nothing was run, in words for the user.
    fn install(&self, release: &Release) -> impl Future<Output = Result<(), String>> + Send;
    /// Opens a page in the browser.
    fn open(&self, url: &str);
}

/// The settings the check follows, from the agent's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Settings {
    auto: bool,
    beta: bool,
    /// No tray icon: an automatic check would have nowhere to say what it found.
    hidden: bool,
}

impl Settings {
    /// `None` while the state has no options to say (no layout loaded yet).
    fn of(state: &Value) -> Option<Settings> {
        Some(Settings {
            auto: state["AutoUpdate"].as_bool()?,
            beta: state["IncludeBetaUpdates"].as_bool().unwrap_or(false),
            hidden: state["HideTrayIcon"] == true,
        })
    }
}

/// Checks for updates while the agent runs: follows its state for the settings, takes
/// `commands` from the tray, sends it `notices`. Ends with the agent, or when the tray
/// is gone (nothing would be left to say what was found).
pub async fn run(
    platform: impl Platform,
    current: Version,
    calls: UnboundedSender<Call>,
    mut commands: UnboundedReceiver<Command>,
    notices: UnboundedSender<Notice>,
) {
    let (client, mut frames) = api::in_process();
    let subscribed = calls.send(Call::Asked {
        id: 0,
        request: Request::Subscribe,
        client,
    });
    if subscribed.is_err() {
        return;
    }
    let mut checker = Checker {
        platform,
        current,
        settings: None,
        found: None,
        announced: None,
        notices,
    };
    let mut next = Instant::now() + FIRST_CHECK;
    loop {
        tokio::select! {
            frame = frames.recv() => {
                let Some(frame) = frame else { return };
                if let Some(settings) = state_of(&frame).as_ref().and_then(Settings::of) {
                    checker.settings = Some(settings);
                }
            }
            command = commands.recv() => {
                let Some(command) = command else { return };
                checker.command(command).await;
            }
            () = tokio::time::sleep_until(next) => {
                next = match checker.settings {
                    // Not known yet: ask again soon rather than a day late.
                    None => Instant::now() + FIRST_CHECK,
                    Some(settings) => {
                        if settings.auto && !settings.hidden {
                            checker.check(false).await;
                        }
                        Instant::now() + DAY
                    }
                };
            }
        }
    }
}

/// The state a frame carries, as `tray::follow` reads it.
fn state_of(frame: &str) -> Option<Value> {
    let mut frame: Value = serde_json::from_str(frame).ok()?;
    if frame["Event"] == "State" {
        Some(frame["State"].take())
    } else if frame["Id"] == 0 {
        Some(frame["Result"].take())
    } else {
        None
    }
}

struct Checker<P> {
    platform: P,
    current: Version,
    settings: Option<Settings>,
    /// The newer release the last check found.
    found: Option<Release>,
    /// The version an automatic check last told about: told once, not every day.
    announced: Option<Version>,
    notices: UnboundedSender<Notice>,
}

impl<P: Platform> Checker<P> {
    async fn command(&mut self, command: Command) {
        match command {
            Command::Check => self.check(true).await,
            Command::Install => {
                let Some(release) = self.found.clone() else {
                    return;
                };
                if let Err(why) = self.platform.install(&release).await {
                    eprintln!(
                        "[lbm-agent] update {} not installed: {why}",
                        release.version
                    );
                    self.tell(Notice {
                        title: "The update was not installed".to_owned(),
                        text: format!("{why}. Click to open the release page."),
                        warning: true,
                        click: Some(Command::OpenPage),
                    });
                }
            }
            Command::OpenPage => {
                if let Some(release) = &self.found {
                    self.platform.open(&release.page);
                }
            }
        }
    }

    /// One check; `manual` when the user asked for it, who is then told everything.
    async fn check(&mut self, manual: bool) {
        let beta = self.settings.unwrap_or_default().beta;
        let releases = match releases(&self.platform).await {
            Ok(releases) => releases,
            Err(why) => {
                eprintln!("[lbm-agent] update check failed: {why}");
                if manual {
                    self.tell(Notice {
                        title: "Could not check for updates".to_owned(),
                        text: why,
                        warning: true,
                        click: None,
                    });
                }
                return;
            }
        };
        self.found = newest(&releases, beta).filter(|r| r.version > self.current);
        let Some(release) = self.found.clone() else {
            eprintln!("[lbm-agent] update check: {} is the newest", self.current);
            if manual {
                self.tell(Notice {
                    title: "Little Big Mouse is up to date".to_owned(),
                    text: format!(
                        "Version {}{}",
                        self.current,
                        if beta { ", betas included" } else { "" }
                    ),
                    warning: false,
                    click: None,
                });
            }
            return;
        };
        eprintln!("[lbm-agent] update check: {} is available", release.version);
        if !manual && self.announced.as_ref() == Some(&release.version) {
            return;
        }
        self.announced = Some(release.version.clone());
        self.tell(Notice {
            title: format!("Little Big Mouse {} is available", release.version),
            text: "Click to download and install it.".to_owned(),
            warning: false,
            click: Some(Command::Install),
        });
    }

    fn tell(&self, notice: Notice) {
        let _ = self.notices.send(notice);
    }
}

#[cfg(test)]
mod tests;
