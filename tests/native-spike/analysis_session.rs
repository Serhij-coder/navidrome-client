use feishin_native_spike::{AnalysisSession, AnalysisSource, AnalysisStatus};
use std::time::{Duration, Instant};

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "capture state did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn startup_failure_latches_until_retry_and_shutdown_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = AnalysisSession::new(
        directory.path().join("missing-ffmpeg"),
        AnalysisSource::TestTone,
        || {},
    )
    .unwrap();
    assert_eq!(session.status(), AnalysisStatus::Suspended);
    session.set_active(true);
    until(|| session.status() == AnalysisStatus::Failed);
    assert!(session.latest().is_none());
    // Disable/enable and explicit retry are both supported; neither hangs after failure.
    session.set_active(false);
    until(|| session.status() == AnalysisStatus::Suspended);
    session.set_active(true);
    until(|| session.status() == AnalysisStatus::Failed);
    session.retry();
    session.shutdown().unwrap();
    session.shutdown().unwrap();
    assert_eq!(session.status(), AnalysisStatus::Stopped);
}

#[cfg(unix)]
#[test]
fn failed_source_is_not_relaunched_without_explicit_retry() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("failed-source.py");
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_capture.py"
        ),
        &binary,
    )
    .unwrap();
    let attempts = || {
        std::fs::read_to_string(binary.with_extension("starts"))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    };
    let mut session =
        AnalysisSession::new(binary.clone(), AnalysisSource::TestTone, || {}).unwrap();
    session.set_active(true);
    until(|| session.status() == AnalysisStatus::Failed);
    assert_eq!(attempts(), 1);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(attempts(), 1, "failed source relaunched automatically");
    session.retry();
    until(|| attempts() == 2 && session.status() == AnalysisStatus::Failed);
    session.set_active(false);
    until(|| session.status() == AnalysisStatus::Suspended);
    session.set_active(true);
    until(|| attempts() == 3 && session.status() == AnalysisStatus::Failed);
    session.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn consumer_suspend_reenable_and_rapid_switch_discard_frames_and_reap_sources() {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("owned-source.py");
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_capture.py"
        ),
        &binary,
    )
    .unwrap();
    let mut session =
        AnalysisSession::new(binary.clone(), AnalysisSource::TestTone, || {}).unwrap();
    for _ in 0..3 {
        session.set_active(true);
        until(|| session.latest().is_some_and(|frame| frame.bands[3] > 0.9));
        session.set_active(false);
        assert!(session.latest().is_none());
        until(|| session.status() == AnalysisStatus::Suspended);
        std::thread::sleep(Duration::from_millis(70));
        assert!(session.latest().is_none());
    }
    for _ in 0..20 {
        session.set_active(true);
        session.set_active(false);
    }
    session.set_active(true);
    until(|| session.latest().is_some());
    let start = Instant::now();
    session.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(session.latest().is_none());
    let starts = std::fs::read_to_string(binary.with_extension("starts")).unwrap();
    assert!(starts.lines().count() >= 4);
    #[cfg(target_os = "linux")]
    for pid in starts.lines() {
        assert!(
            !std::path::Path::new("/proc").join(pid).exists(),
            "source not reaped"
        );
    }
}
