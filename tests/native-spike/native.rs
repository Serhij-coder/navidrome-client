use feishin_native_spike::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
fn tone_bounds_no_overwrite_and_logs_exclude_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owned.wav");
    assert!(write_tone(&path, 0, 440).is_err());
    write_tone(&path, 1, 440).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 44 + 48_000 * 2);
    assert!(write_tone(&path, 1, 440).is_err());
    let event = log_event(LogEvent::Failed, 3);
    assert_eq!(event.as_object().unwrap().len(), 3);
    assert_eq!(event["generation"], 3);
    let error = Error::from(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "https://secret.example/?token=credential",
    ));
    assert!(!error.to_string().contains("credential"));
}

#[cfg(unix)]
fn fake(directory: &Path, mode: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let executable = directory.join(mode);
    std::fs::write(&executable, include_str!("fake_mpv.py")).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

#[cfg(unix)]
#[test]
fn ipc_correlation_generation_queue_and_bounded_stalled_quit() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("tone.wav");
    write_tone(&tone, 1, 440).unwrap();
    let executable = fake(directory.path(), "stalled-quit");
    let mut mpv = Mpv::start(&executable, 9, AudioOutput::Null).unwrap();
    let socket_directory = mpv.socket_directory().to_owned();
    mpv.load_local(&tone, false).unwrap();
    mpv.load_local(&tone, true).unwrap();
    assert_eq!(mpv.playlist_count().unwrap(), 2);
    assert!(mpv.accepts_generation(9));
    assert!(!mpv.accepts_generation(8));
    assert!(mpv.seek(f64::NAN).is_err());
    let start = Instant::now();
    mpv.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(!mpv.accepts_generation(9));
    drop(mpv);
    assert!(!socket_directory.exists());
}

#[cfg(unix)]
#[test]
fn malformed_oversized_late_reply_and_process_death_are_errors() {
    let directory = tempfile::tempdir().unwrap();
    for mode in ["malformed", "oversized", "wrong-id", "die"] {
        let executable = fake(directory.path(), mode);
        let mut mpv = Mpv::start(&executable, 0, AudioOutput::Null).unwrap();
        assert!(mpv.playlist_count().is_err(), "{mode}");
        mpv.shutdown().unwrap();
    }
    assert!(Mpv::start(&directory.path().join("missing"), 1, AudioOutput::Null).is_err());
}

#[test]
fn capture_rejects_external_source_option_injection() {
    for source in [
        "",
        "-i",
        "server;command",
        "https://credential@server",
        "a b",
        "../private",
    ] {
        assert!(Capture::start_monitor(Path::new("unused"), source).is_err());
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires external mpv; null output verifies IPC, not audio hardware"]
fn real_mpv_current_next_pause_seek_filter_and_process_death() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("tone.wav");
    write_tone(&tone, 5, 750).unwrap();
    let binary = std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into());
    let mut mpv = Mpv::start(Path::new(&binary), 5, AudioOutput::Null).unwrap();
    mpv.load_local(&tone, false).unwrap();
    mpv.load_local(&tone, true).unwrap();
    assert_eq!(mpv.playlist_count().unwrap(), 2);
    std::thread::sleep(Duration::from_millis(250));
    mpv.pause(true).unwrap();
    mpv.seek(1.0).unwrap();
    mpv.low_pass_probe().unwrap();
    mpv.pause(false).unwrap();
    assert!(mpv.position().unwrap() >= 0.9);
    mpv.terminate_for_probe().unwrap();
    assert!(!mpv.accepts_generation(5));
    assert!(mpv.position().is_err());
}

#[test]
#[ignore = "requires external FFmpeg; generated source is not OS capture"]
fn real_ffmpeg_analysis_and_disposal() {
    let mut capture = Capture::start_test_tone(Path::new("ffmpeg")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let frame = loop {
        if let Some(frame) = capture.latest() {
            break frame;
        }
        assert!(
            Instant::now() < deadline,
            "no analysis frame before deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(frame.bands.iter().any(|band| *band > 0.01));
    capture.shutdown().unwrap();
    assert!(!capture.is_running().unwrap());
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit isolated p01_owned PulseAudio sink, external mpv and FFmpeg"]
fn owned_mpv_monitor_capture_and_teardown() {
    assert_eq!(
        std::env::var("P01_CAPTURE_MONITOR").as_deref(),
        Ok("p01_owned.monitor"),
        "must explicitly select the disposable owned-tone monitor"
    );
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 5, 750).unwrap();
    let binary = std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into());
    let mut capture = Capture::start_monitor(Path::new("ffmpeg"), "p01_owned.monitor").unwrap();
    let mut mpv = Mpv::start(Path::new(&binary), 7, AudioOutput::Pulse).unwrap();
    mpv.load_local(&tone, false).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(frame) = capture.latest()
            && frame.bands.iter().any(|band| *band > 0.02)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no owned audio reached monitor analysis"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // Simulate consumer close/minimize and engine switch: capture is disposed first.
    capture.shutdown().unwrap();
    assert!(!capture.is_running().unwrap());
    assert!(mpv.accepts_generation(7));
    mpv.shutdown().unwrap();
    assert!(!mpv.accepts_generation(7));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires external FFmpeg; unavailable disposable source, not OS permission-prompt evidence"]
fn unavailable_monitor_exits_without_a_frame() {
    let source = format!("p01_missing_{}.monitor", std::process::id());
    let mut capture = Capture::start_monitor(Path::new("ffmpeg"), &source).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while capture.is_running().unwrap() {
        assert!(
            Instant::now() < deadline,
            "unavailable monitor did not fail within probe deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(capture.latest().is_none());
    capture.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn slow_partial_replies_obey_the_absolute_deadline() {
    let directory = tempfile::tempdir().unwrap();
    let executable = fake(directory.path(), "slow-drip");
    let mut mpv = Mpv::start(&executable, 0, AudioOutput::Null).unwrap();
    let start = Instant::now();
    assert!(matches!(mpv.playlist_count(), Err(Error::Deadline)));
    assert!(start.elapsed() < Duration::from_secs(4));
    mpv.shutdown().unwrap();
    let executable = fake(directory.path(), "slow-start");
    let start = Instant::now();
    assert!(matches!(
        Mpv::start(&executable, 0, AudioOutput::Null),
        Err(Error::Deadline)
    ));
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[cfg(unix)]
#[test]
fn non_unicode_local_file_is_a_controlled_input_error() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(OsString::from_vec(b"private-\xff.wav".to_vec()));
    std::fs::write(&path, b"owned fixture").unwrap();
    let executable = fake(directory.path(), "normal");
    let mut mpv = Mpv::start(&executable, 0, AudioOutput::Null).unwrap();
    assert!(matches!(
        mpv.load_local(&path, false),
        Err(Error::InvalidInput)
    ));
}
