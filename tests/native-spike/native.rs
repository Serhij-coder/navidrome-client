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
    let executable = directory.join(mode);
    // Never execute a just-written file: another parallel fork can transiently
    // inherit its write descriptor and make exec fail with ETXTBSY.
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_mpv.py"
        ),
        &executable,
    )
    .unwrap();
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
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&socket_directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
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
fn alternative_engine_owns_child_and_rejects_invalid_seek() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("tone.wav");
    write_tone(&tone, 1, 440).unwrap();
    let executable = fake(directory.path(), "ffplay-stall");
    for seconds in [f64::NAN, f64::INFINITY, -1.0, 86_401.0] {
        assert!(matches!(
            FfplayProbe::start_local(&executable, &tone, seconds, 4, FfplayOutput::Dummy),
            Err(Error::InvalidInput)
        ));
    }
    let mut probe =
        FfplayProbe::start_local(&executable, &tone, 0.1, 4, FfplayOutput::Dummy).unwrap();
    assert!(probe.accepts_generation(4));
    assert!(!probe.accepts_generation(3));
    let start = Instant::now();
    probe.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(!probe.accepts_generation(4));
    assert_eq!(probe.poll().unwrap(), Some(false));
}

#[test]
#[ignore = "requires external ffplay with SDL dummy driver; decode/lifecycle only, no physical audio"]
fn real_alternative_engine_after_mpv_startup_failure_and_initial_seek() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("tone.wav");
    write_tone(&tone, 2, 750).unwrap();
    #[cfg(unix)]
    assert!(Mpv::start(&directory.path().join("missing-mpv"), 1, AudioOutput::Null).is_err());
    let mut alternative =
        FfplayProbe::start_local(Path::new("ffplay"), &tone, 1.0, 2, FfplayOutput::Dummy).unwrap();
    assert!(!alternative.accepts_generation(1));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(success) = alternative.poll().unwrap() {
            assert!(success);
            break;
        }
        assert!(Instant::now() < deadline, "ffplay decode did not complete");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alternative.accepts_generation(2));
    alternative.shutdown().unwrap();
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
    write_tone(&tone, 30, 750).unwrap();
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
    let mut supervised = AnalysisSession::new(
        "ffmpeg".into(),
        AnalysisSource::Monitor("p01_owned.monitor".into()),
        || {},
    )
    .unwrap();
    for _ in 0..2 {
        supervised.set_active(true);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if supervised
                .latest()
                .is_some_and(|frame| frame.bands.iter().any(|band| *band > 0.02))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "supervised monitor did not resume"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        supervised.set_active(false);
        assert!(supervised.latest().is_none());
        let deadline = Instant::now() + Duration::from_secs(2);
        while supervised.status() != AnalysisStatus::Suspended {
            assert!(Instant::now() < deadline, "monitor did not suspend");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(mpv.position().unwrap() > 0.0);
    }
    supervised.shutdown().unwrap();
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
    // Linux also exercises a real existing byte filename. Other Unix filesystems
    // need not support creating that name to verify rejection at the IPC boundary.
    #[cfg(target_os = "linux")]
    std::fs::write(&path, b"owned fixture").unwrap();
    let executable = fake(directory.path(), "normal");
    let mut mpv = Mpv::start(&executable, 0, AudioOutput::Null).unwrap();
    assert!(matches!(
        mpv.load_local(&path, false),
        Err(Error::InvalidInput)
    ));
    // The same wire-contract error applies before any filesystem lookup.
    assert!(matches!(
        mpv.load_local(&path.with_extension("missing"), false),
        Err(Error::InvalidInput)
    ));
    assert!(mpv.accepts_generation(0));
    assert_eq!(mpv.playlist_count().unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn long_temp_root_keeps_ipc_path_short_and_private() {
    use std::{
        os::unix::{ffi::OsStrExt, fs::PermissionsExt},
        process::Command,
    };
    if std::env::var_os("P01_LONG_TMP_CHILD").is_some() {
        let directory = tempfile::tempdir().unwrap();
        let executable = fake(directory.path(), "normal");
        let mut mpv = Mpv::start(&executable, 1, AudioOutput::Null).unwrap();
        let socket_dir = mpv.socket_directory().to_owned();
        assert!(socket_dir.join("ipc").as_os_str().as_bytes().len() <= 100);
        assert_eq!(
            std::fs::metadata(&socket_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(mpv.playlist_count().unwrap(), 0);
        mpv.shutdown().unwrap();
        drop(mpv);
        assert!(!socket_dir.exists());
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let long_root = directory.path().join("owned-long-temp-root-".repeat(6));
    std::fs::create_dir(&long_root).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "long_temp_root_keeps_ipc_path_short_and_private",
            "--nocapture",
        ])
        .env("TMPDIR", &long_root)
        .env("P01_LONG_TMP_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "long-root subprocess: {} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(unix)]
#[test]
fn volume_percentage_validation_leaves_the_engine_usable() {
    let directory = tempfile::tempdir().unwrap();
    let executable = fake(directory.path(), "normal");
    let mut mpv = Mpv::start(&executable, 11, AudioOutput::Null).unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 100.1] {
        assert!(matches!(
            mpv.set_volume_percent(invalid),
            Err(Error::InvalidInput)
        ));
        assert!(mpv.accepts_generation(11));
    }
    for valid in [0.0, 0.5, 50.0, 100.0] {
        mpv.set_volume_percent(valid).unwrap();
    }
    assert_eq!(mpv.playlist_count().unwrap(), 0);
}
