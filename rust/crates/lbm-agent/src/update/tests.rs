use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// An entry of the API's list, as GitHub writes it.
fn entry(name: &str, tag: &str, prerelease: bool) -> Value {
    json!({
        "name": name,
        "tag_name": tag,
        "draft": false,
        "prerelease": prerelease,
        "html_url": format!("https://github.com/mgth/LittleBigMouse/releases/tag/{tag}"),
        "assets": [{
            "name": format!("LittleBigMouse_{}.exe", tag.trim_start_matches('v')),
            "size": 12_345_678,
            "browser_download_url": format!(
                "https://github.com/mgth/LittleBigMouse/releases/download/{tag}/LittleBigMouse_{}.exe",
                tag.trim_start_matches('v')
            ),
        }],
    })
}

#[test]
fn the_versions_the_releases_have_been_named_with_are_read() {
    assert_eq!(parse_version("5.6.0"), Some(v("5.6.0")));
    assert_eq!(parse_version("v5.3.0"), Some(v("5.3.0")));
    assert_eq!(
        parse_version("5.2.3.0"),
        Some(v("5.2.3")),
        "the old four-part names"
    );
    assert_eq!(parse_version(" 6.0.0-beta.1 "), Some(v("6.0.0-beta.1")));
    assert_eq!(parse_version("V6.1"), Some(v("6.1.0")));
    assert_eq!(parse_version("Little Big Mouse 5.3.0-beta.1"), None);
    assert_eq!(parse_version(""), None);
    assert_eq!(parse_version("1.2.3.4.5"), None);
}

#[test]
fn the_newest_stable_release_wins_whatever_the_order_of_the_list() {
    let list = json!([
        entry("5.5.1", "v5.5.1", false),
        entry("5.6.0", "v5.6.0", false),
        entry("5.2.3.0", "v5.2.3", false),
    ]);
    let release = newest(&list, false).unwrap();
    assert_eq!(release.version, v("5.6.0"));
    assert_eq!(release.file_name, "LittleBigMouse_5.6.0.exe");
    assert_eq!(release.size, 12_345_678);
    assert_eq!(
        release.page,
        "https://github.com/mgth/LittleBigMouse/releases/tag/v5.6.0"
    );
}

#[test]
fn betas_are_offered_only_to_whoever_asked_for_them() {
    let list = json!([
        entry("Little Big Mouse 6.0.0-beta.2", "v6.0.0-beta.2", true),
        entry("5.6.0", "v5.6.0", false),
    ]);
    assert_eq!(newest(&list, false).unwrap().version, v("5.6.0"));
    // The name does not parse: the tag gives the version, as in C#.
    assert_eq!(newest(&list, true).unwrap().version, v("6.0.0-beta.2"));

    // A release published as stable but named as a beta is a beta all the same.
    let mislabelled = json!([entry("6.0.0-rc.1", "v6.0.0-rc.1", false)]);
    assert_eq!(newest(&mislabelled, false), None);
}

#[test]
fn drafts_and_releases_without_an_installer_are_skipped() {
    let mut draft = entry("7.0.0", "v7.0.0", false);
    draft["draft"] = json!(true);
    let mut bare = entry("6.5.0", "v6.5.0", false);
    bare["assets"] = json!([]);
    let mut zip = entry("6.4.0", "v6.4.0", false);
    zip["assets"][0]["name"] = json!("LittleBigMouse_6.4.0.zip");
    let list = json!([draft, bare, zip, entry("6.0.0", "v6.0.0", false)]);
    assert_eq!(newest(&list, true).unwrap().version, v("6.0.0"));
}

#[test]
fn an_installer_from_elsewhere_or_with_a_strange_name_is_refused() {
    let mut elsewhere = entry("6.1.0", "v6.1.0", false);
    elsewhere["assets"][0]["browser_download_url"] =
        json!("https://example.com/mgth/LittleBigMouse/releases/download/v6.1.0/x.exe");
    let mut path = entry("6.2.0", "v6.2.0", false);
    path["assets"][0]["name"] = json!("..\\..\\Startup\\evil.exe");
    let mut page = entry("6.3.0", "v6.3.0", false);
    page["html_url"] = json!("https://example.com/");
    let list = json!([elsewhere, path, page, entry("6.0.0", "v6.0.0", false)]);
    assert_eq!(newest(&list, false).unwrap().version, v("6.0.0"));

    // GitHub's spelling of the owner is not the repository's: case does not matter.
    let mut upper = entry("6.4.0", "v6.4.0", false);
    upper["assets"][0]["browser_download_url"] = json!(
        "https://github.com/Mgth/LittleBigMouse/releases/download/v6.4.0/LittleBigMouse_6.4.0.exe"
    );
    assert_eq!(newest(&json!([upper]), false).unwrap().version, v("6.4.0"));
}

#[test]
fn a_beta_is_older_than_its_release() {
    // What lets a beta tester with betas off still hear of the stable release.
    assert!(v("6.0.0") > v("6.0.0-beta.2"));
    assert!(v("6.0.0-beta.2") > v("6.0.0-beta.1"));
    assert!(v("6.0.0-beta.1") > v("5.6.0"));
}

#[test]
fn this_build_has_a_version_to_compare() {
    assert!(current().is_some(), "{}", env!("LBM_RELEASE_VERSION"));
}

/// A network and a shell that only remember what they were asked.
#[derive(Clone)]
struct Fake {
    releases: Arc<Mutex<Result<Value, String>>>,
    fetched: Arc<Mutex<u32>>,
    installed: Arc<Mutex<Vec<Version>>>,
    install_fails: Arc<Mutex<Option<String>>>,
    opened: Arc<Mutex<Vec<String>>>,
}

impl Default for Fake {
    fn default() -> Self {
        Fake {
            releases: Arc::new(Mutex::new(Ok(json!([])))),
            fetched: Default::default(),
            installed: Default::default(),
            install_fails: Default::default(),
            opened: Default::default(),
        }
    }
}

impl Platform for Fake {
    fn releases(&self) -> impl Future<Output = Result<Value, String>> + Send {
        *self.fetched.lock().unwrap() += 1;
        let answer = self.releases.lock().unwrap().clone();
        async move { answer }
    }

    fn install(&self, release: &Release) -> impl Future<Output = Result<(), String>> + Send {
        self.installed.lock().unwrap().push(release.version.clone());
        let answer = match self.install_fails.lock().unwrap().clone() {
            Some(why) => Err(why),
            None => Ok(()),
        };
        async move { answer }
    }

    fn open(&self, url: &str) {
        self.opened.lock().unwrap().push(url.to_owned());
    }
}

struct Harness {
    fake: Fake,
    commands: UnboundedSender<Command>,
    notices: UnboundedReceiver<Notice>,
    /// The agent's side: where the check subscribed, to send it states.
    agent: api::Client,
    _calls: UnboundedReceiver<Call>,
}

impl Harness {
    async fn start(current: &str, releases: Value) -> Harness {
        let fake = Fake::default();
        *fake.releases.lock().unwrap() = Ok(releases);
        let (calls, mut calls_rx) = tokio::sync::mpsc::unbounded_channel();
        let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notices_tx, notices) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(run(
            fake.clone(),
            v(current),
            calls,
            commands_rx,
            notices_tx,
        ));
        let Some(Call::Asked {
            id: 0,
            request: Request::Subscribe,
            client,
        }) = calls_rx.recv().await
        else {
            panic!("the check subscribes to the state first");
        };
        Harness {
            fake,
            commands,
            notices,
            agent: client,
            _calls: calls_rx,
        }
    }

    fn state(&self, state: Value) {
        self.agent
            .send(json!({ "Event": "State", "State": state }).to_string());
    }

    /// Lets the loop run until it waits again.
    async fn settle(&self) {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    fn fetched(&self) -> u32 {
        *self.fake.fetched.lock().unwrap()
    }
}

#[tokio::test(start_paused = true)]
async fn the_automatic_check_runs_a_minute_in_then_daily_and_tells_once() {
    let mut h = Harness::start("5.6.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    h.state(json!({ "Engine": "Running", "AutoUpdate": true }));
    h.settle().await;
    assert_eq!(h.fetched(), 0, "not before the first minute");

    tokio::time::advance(FIRST_CHECK).await;
    h.settle().await;
    assert_eq!(h.fetched(), 1);
    let notice = h.notices.try_recv().expect("a newer version is told");
    assert_eq!(notice.title, "Little Big Mouse 6.0.0 is available");
    assert_eq!(notice.click, Some(Command::Install));

    tokio::time::advance(DAY / 2).await;
    h.settle().await;
    assert_eq!(h.fetched(), 1, "nothing in between");

    tokio::time::advance(DAY / 2).await;
    h.settle().await;
    assert_eq!(h.fetched(), 2, "a day later");
    assert!(
        h.notices.try_recv().is_err(),
        "the same version is not told twice"
    );

    // A newer one is.
    *h.fake.releases.lock().unwrap() = Ok(json!([entry("6.0.1", "v6.0.1", false)]));
    tokio::time::advance(DAY).await;
    h.settle().await;
    assert_eq!(
        h.notices.try_recv().unwrap().title,
        "Little Big Mouse 6.0.1 is available"
    );
}

#[tokio::test(start_paused = true)]
async fn no_automatic_check_is_made_when_the_option_is_off() {
    let h = Harness::start("5.6.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    h.state(json!({ "Engine": "Running", "AutoUpdate": false }));
    for _ in 0..3 {
        tokio::time::advance(DAY).await;
        h.settle().await;
    }
    assert_eq!(h.fetched(), 0);
}

#[tokio::test(start_paused = true)]
async fn no_automatic_check_is_made_without_a_tray_icon_to_tell() {
    let h = Harness::start("5.6.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    h.state(json!({ "AutoUpdate": true, "HideTrayIcon": true }));
    tokio::time::advance(FIRST_CHECK).await;
    h.settle().await;
    tokio::time::advance(DAY).await;
    h.settle().await;
    assert_eq!(h.fetched(), 0);

    // The icon back: the next day's check is made.
    h.state(json!({ "AutoUpdate": true, "HideTrayIcon": false }));
    h.settle().await;
    tokio::time::advance(DAY).await;
    h.settle().await;
    assert_eq!(h.fetched(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_first_check_waits_for_the_options() {
    let mut h = Harness::start("5.6.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    // An agent with no layout yet says nothing of the options.
    h.state(json!({ "Engine": "Dead" }));
    tokio::time::advance(FIRST_CHECK).await;
    h.settle().await;
    assert_eq!(h.fetched(), 0);

    h.state(json!({ "Engine": "Running", "AutoUpdate": true }));
    h.settle().await;
    tokio::time::advance(FIRST_CHECK).await;
    h.settle().await;
    assert_eq!(h.fetched(), 1, "a minute after the retry, not a day");
    assert!(h.notices.try_recv().is_ok());
}

#[tokio::test(start_paused = true)]
async fn the_tray_entry_checks_even_with_the_option_off_and_says_up_to_date() {
    let mut h = Harness::start("6.0.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    h.state(json!({ "Engine": "Running", "AutoUpdate": false }));
    h.commands.send(Command::Check).unwrap();
    h.settle().await;
    assert_eq!(h.fetched(), 1);
    let notice = h.notices.try_recv().unwrap();
    assert_eq!(notice.title, "Little Big Mouse is up to date");
    assert_eq!(notice.text, "Version 6.0.0");
    assert_eq!(notice.click, None);
}

#[tokio::test(start_paused = true)]
async fn the_beta_option_reaches_the_check() {
    let list = json!([
        entry("6.1.0-beta.1", "v6.1.0-beta.1", true),
        entry("6.0.0", "v6.0.0", false)
    ]);
    let mut h = Harness::start("6.0.0", list).await;
    h.state(json!({ "AutoUpdate": false, "IncludeBetaUpdates": false }));
    h.settle().await;
    h.commands.send(Command::Check).unwrap();
    h.settle().await;
    assert_eq!(
        h.notices.try_recv().unwrap().title,
        "Little Big Mouse is up to date"
    );

    h.state(json!({ "AutoUpdate": false, "IncludeBetaUpdates": true }));
    // The loop takes what is ready in no set order: the state first, as in use, where
    // the option is set long before the menu is opened.
    h.settle().await;
    h.commands.send(Command::Check).unwrap();
    h.settle().await;
    assert_eq!(
        h.notices.try_recv().unwrap().title,
        "Little Big Mouse 6.1.0-beta.1 is available"
    );
}

#[tokio::test(start_paused = true)]
async fn a_click_installs_what_was_found_and_a_refused_installer_offers_the_page() {
    let mut h = Harness::start("5.6.0", json!([entry("6.0.0", "v6.0.0", false)])).await;
    h.state(json!({ "AutoUpdate": false }));
    // Nothing found yet: nothing to install.
    h.commands.send(Command::Install).unwrap();
    h.settle().await;
    assert!(h.fake.installed.lock().unwrap().is_empty());

    h.commands.send(Command::Check).unwrap();
    h.settle().await;
    let _ = h.notices.try_recv().unwrap();
    *h.fake.install_fails.lock().unwrap() =
        Some("The installer is not signed by SignPath Foundation".to_owned());
    h.commands.send(Command::Install).unwrap();
    h.settle().await;
    assert_eq!(*h.fake.installed.lock().unwrap(), [v("6.0.0")]);
    let notice = h.notices.try_recv().unwrap();
    assert!(notice.warning);
    assert_eq!(
        notice.text,
        "The installer is not signed by SignPath Foundation. Click to open the release page."
    );
    assert_eq!(notice.click, Some(Command::OpenPage));

    h.commands.send(Command::OpenPage).unwrap();
    h.settle().await;
    assert_eq!(
        *h.fake.opened.lock().unwrap(),
        ["https://github.com/mgth/LittleBigMouse/releases/tag/v6.0.0"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_check_is_said_only_when_asked_for() {
    let mut h = Harness::start("5.6.0", json!([])).await;
    *h.fake.releases.lock().unwrap() = Err("HTTP 403".to_owned());
    h.state(json!({ "AutoUpdate": true }));
    tokio::time::advance(FIRST_CHECK).await;
    h.settle().await;
    assert_eq!(h.fetched(), 1);
    assert!(
        h.notices.try_recv().is_err(),
        "an automatic check fails quietly"
    );

    h.commands.send(Command::Check).unwrap();
    h.settle().await;
    let notice = h.notices.try_recv().unwrap();
    assert_eq!(
        (notice.title.as_str(), notice.text.as_str()),
        ("Could not check for updates", "HTTP 403")
    );
}

#[tokio::test(start_paused = true)]
async fn the_check_ends_when_the_tray_goes() {
    let fake = Fake::default();
    let (calls, mut calls_rx) = tokio::sync::mpsc::unbounded_channel();
    let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
    let (notices_tx, _notices) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(run(fake, v("5.6.0"), calls, commands_rx, notices_tx));
    let _subscription = calls_rx.recv().await;
    drop(commands);
    task.await.unwrap();
}
