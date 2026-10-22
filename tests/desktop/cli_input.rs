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
