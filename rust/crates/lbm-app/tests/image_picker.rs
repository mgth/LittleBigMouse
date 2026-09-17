use lbm_app::image_picker::validate;
#[test]
fn only_decodable_absolute_local_files_enter_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("été 100% #.PNG");
    image::RgbaImage::from_pixel(3, 2, image::Rgba([20, 30, 40, 255]))
        .save(&path)
        .unwrap();
    assert_eq!(validate(path.clone()).unwrap(), path.to_str().unwrap());
    assert!(validate(dir.path().to_owned()).is_err());
    assert!(validate(dir.path().join("missing.png")).is_err());
    assert!(validate("relative.png".into()).is_err());
    std::fs::write(&path, "bad image").unwrap();
    assert!(validate(path).is_err());
}
#[cfg(unix)]
#[test]
fn non_unicode_filenames_are_rejected_instead_of_silently_changed() {
    use std::os::unix::ffi::OsStringExt;
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/invalid-\xff.png".to_vec(),
    ));
    assert!(validate(path)
        .unwrap_err()
        .contains("cannot be represented"));
}
#[cfg(target_os = "linux")]
#[test]
fn an_unavailable_portal_is_an_error_not_user_cancellation() {
    if std::env::var_os("LBM_TEST_ISOLATED_PICKER").is_some() {
        let result = lbm_app::image_picker::choose(lbm_app::image_picker::Parent::None, None);
        assert!(result.unwrap_err().contains("file chooser is unavailable"));
        return;
    }
    // A child receives an unreachable bus: no test can open a dialog on the real desktop.
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "an_unavailable_portal_is_an_error_not_user_cancellation",
            "--nocapture",
        ])
        .env("LBM_TEST_ISOLATED_PICKER", "1")
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", dir.path().join("no-bus").display()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
