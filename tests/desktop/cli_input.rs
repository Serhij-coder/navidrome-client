#[cfg(unix)]
#[test]
fn invalid_unicode_argument_fails_without_panicking_or_echoing_input() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, process::Command};
    let output = Command::new(env!("CARGO_BIN_EXE_feishin-desktop"))
        .arg(OsString::from_vec(b"private-\xff-path".to_vec()))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"));
    assert!(!stderr.contains("private-"));
}

#[test]
fn profile_root_rejects_missing_relative_parent_and_duplicate_without_echo() {
    use std::process::Command;
    for args in [
        vec!["--profile-root"],
        vec!["--profile-root", "private-relative"],
        vec!["--profile-root", "/private/../path"],
        vec!["--profile-root", "/private", "--profile-root", "/other"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_feishin-desktop"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("private"));
        assert!(!stderr.contains("panicked"));
    }
}
