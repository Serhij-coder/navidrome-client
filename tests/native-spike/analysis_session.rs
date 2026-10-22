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
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("failed-source.py");
    std::fs::write(
        &binary,
        r#"#!/usr/bin/env python3
import pathlib, sys
with pathlib.Path(__file__).with_suffix('.starts').open('a') as out:
    out.write('started\n')
sys.exit(1)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
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
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("owned-source.py");
    std::fs::write(
        &binary,
        r#"#!/usr/bin/env python3
import math, os, pathlib, struct, sys, time
with pathlib.Path(__file__).with_suffix('.starts').open('a') as out:
    out.write(str(os.getpid()) + '\n')
samples = b''.join(struct.pack('<f', math.sin(i * math.tau * 4 / 256)) for i in range(256))
while True:
    sys.stdout.buffer.write(samples)
    sys.stdout.buffer.flush()
    time.sleep(0.032)
"#,
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
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
