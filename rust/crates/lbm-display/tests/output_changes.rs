//! Real command generation/validation with a compositor double, never the test desktop.
use lbm_display::linux::{
    display_signature,
    topology::{change_checked_with, Change},
    xrandr, Backend,
};
use lbm_layout::linux::LinuxMonitor;
use std::cell::Cell;

fn output(name: &str, enabled: bool, primary: bool) -> LinuxMonitor {
    LinuxMonitor {
        connector_name: name.into(),
        enabled,
        primary,
        ..LinuxMonitor::fallback()
    }
}

#[test]
fn connected_but_disabled_xrandr_outputs_can_be_found_again() {
    let text = "DP-1 connected primary 1920x1080+0+0\nHDMI-1 connected (normal left inverted right x axis y axis)\nDP-2 disconnected (normal)\nBAD connected 99999999999999x1+0+0";
    let live = xrandr::parse_connected(text, &Default::default());
    assert_eq!(live.len(), 2);
    assert!(live[0].enabled);
    assert_eq!(live[1].connector_name, "HDMI-1");
    assert!(!live[1].enabled);
    assert!(!live[1].primary);
    assert_eq!(
        xrandr::parse(text, &Default::default()).len(),
        1,
        "layout discovery is unchanged"
    );
}

#[test]
fn outputs_left_or_above_the_origin_remain_enabled_in_the_inventory() {
    let live = xrandr::parse_connected(
        "DP-1 connected 1920x1080-1920-1080\nDP-2 connected primary 1920x1080+0+0",
        &Default::default(),
    );
    assert_eq!(live.len(), 2);
    assert!(live.iter().all(|m| m.enabled));
    assert_eq!((live[0].logical_x, live[0].logical_y), (-1920.0, -1080.0));
}

#[test]
fn stale_missing_ambiguous_and_last_screen_actions_never_run_a_command() {
    let one = vec![output("DP-1", true, true)];
    for (before, target, action, signature) in [
        (one.clone(), "DP-1", Change::Detach, display_signature(&one)),
        (
            one.clone(),
            "missing",
            Change::Attach,
            display_signature(&one),
        ),
        (one.clone(), "DP-1", Change::Primary, "stale".into()),
        (
            {
                let mut v = one.clone();
                v.push(output("HDMI-1", false, false));
                v
            },
            "HDMI-1",
            Change::Primary,
            String::new(),
        ),
        (
            vec![one[0].clone(), one[0].clone()],
            "DP-1",
            Change::Detach,
            String::new(),
        ),
    ] {
        let signature = if signature.is_empty() {
            display_signature(&before)
        } else {
            signature
        };
        let result = change_checked_with(
            Backend::XRandR,
            action,
            target,
            &signature,
            || Ok(before.clone()),
            |_, _| panic!("must refuse before invoking the compositor"),
        );
        assert!(result.is_err());
    }
}

#[test]
fn each_change_is_checked_against_the_state_reported_after_the_command() {
    for backend in [Backend::KScreen, Backend::XRandR] {
        for action in [Change::Primary, Change::Attach, Change::Detach] {
            let before = vec![
                output("DP-1", true, true),
                output("HDMI-1", action != Change::Attach, false),
            ];
            let mut after = before.clone();
            match action {
                Change::Primary => {
                    after[0].primary = false;
                    after[1].primary = true;
                }
                Change::Attach => after[1].enabled = true,
                Change::Detach => after[1].enabled = false,
            }
            let sent = Cell::new(false);
            let actual = change_checked_with(
                backend,
                action,
                "HDMI-1",
                &display_signature(&before),
                || {
                    Ok(if sent.get() {
                        after.clone()
                    } else {
                        before.clone()
                    })
                },
                |program, args| {
                    let (expected, arguments) =
                        lbm_display::linux::topology::command(backend, action, "HDMI-1");
                    assert_eq!(program, expected);
                    assert_eq!(args, arguments);
                    sent.set(true);
                    Ok(())
                },
            )
            .unwrap();
            assert!(sent.get());
            assert_eq!(actual, after);
        }
    }
}

#[test]
fn a_successful_exit_without_a_changed_state_is_not_success() {
    let before = vec![output("DP-1", true, true), output("HDMI-1", true, false)];
    let result = change_checked_with(
        Backend::XRandR,
        Change::Detach,
        "HDMI-1",
        &display_signature(&before),
        || Ok(before.clone()),
        |_, _| Ok(()),
    );
    assert!(result.unwrap_err().contains("did not confirm"));
    let failed = change_checked_with(
        Backend::XRandR,
        Change::Detach,
        "HDMI-1",
        &display_signature(&before),
        || Ok(before.clone()),
        |_, _| Err("compositor refused".into()),
    );
    assert_eq!(failed.unwrap_err(), "compositor refused");
}
