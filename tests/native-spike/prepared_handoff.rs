#![cfg(any(unix, windows))]
use feishin_native_spike::*;
use std::time::Instant;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
const WAIT: Duration = Duration::from_secs(10);
fn with_session(check: impl FnOnce(&HandoffSession)) {
    let session = HandoffSession::start().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&session)));
    session.shutdown();
    let cleanup = session.wait_shutdown(WAIT);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
    cleanup.unwrap();
}
fn preparation(source: &Path, paused: bool) -> LocalPreparation {
    LocalPreparation {
        source: source.into(),
        position_seconds: 1.25,
        paused,
        volume_percent: 37.0,
    }
}
#[cfg(unix)]
fn submit(
    session: &HandoffSession,
    binary: PathBuf,
    generation: u64,
    source: &Path,
    budget: Duration,
) -> HandoffRequest {
    session
        .replace_prepared(
            binary,
            generation,
            AudioOutput::Null,
            preparation(source, true),
            budget,
        )
        .unwrap()
}
fn command(client: &ControlClient, generation: u64, command: ControlCommand) -> ControlValue {
    client
        .submit(generation, 100, Duration::from_secs(3), command)
        .unwrap()
        .wait(WAIT)
        .unwrap()
        .result
        .unwrap()
}
#[cfg(unix)]
fn fake(dir: &Path, name: &str) -> PathBuf {
    std::fs::write(dir.join("owned.wav"), []).unwrap();
    let path = dir.join(name);
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/prepared_handoff_peer.py"
        ),
        &path,
    )
    .unwrap();
    path
}
#[cfg(unix)]
fn gate(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.exists() {
        assert!(Instant::now() < deadline, "gate {}", path.display());
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[cfg(unix)]
fn trace(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
#[cfg(unix)]
fn no_overlap(dir: &Path) {
    assert!(!trace(dir).iter().any(|e| e["event"] == "overlap"));
}
#[cfg(unix)]
#[test]
fn readiness_and_seek_arrival_gate_publication_and_seek_is_sent_once() {
    for mode in ["ready", "seekgate"] {
        let dir = tempfile::tempdir().unwrap();
        with_session(|session| {
            let request = submit(
                session,
                fake(dir.path(), mode),
                1,
                &dir.path().join("owned.wav"),
                WAIT,
            );
            gate(&dir.path().join(if mode == "ready" {
                "position-ready"
            } else {
                "seek-seekgate"
            }));
            assert!(request.try_reply().unwrap().is_none());
            let release = if mode == "ready" {
                "release-ready-ready"
            } else {
                "release-seek-seekgate"
            };
            std::fs::write(dir.path().join(release), []).unwrap();
            let client = request.wait(WAIT).unwrap().result.unwrap();
            assert!(matches!(
                command(&client, 1, ControlCommand::PlaybackStatus),
                ControlValue::PlaybackStatus(NativePlaybackStatus::Paused)
            ));
            assert!(
                matches!(command(&client, 1, ControlCommand::VolumePercent), ControlValue::VolumePercent(v) if v == 37.0)
            );
        });
        let events = trace(dir.path());
        assert_eq!(
            events.iter().filter(|e| e["command"][0] == "seek").count(),
            1
        );
        let mutations: Vec<_> = events
            .iter()
            .filter_map(|e| e["command"].as_array())
            .filter(|c| c[0] == "set_property" || c[0] == "loadfile" || c[0] == "seek")
            .collect();
        assert_eq!(
            mutations[0],
            &vec![
                serde_json::json!("set_property"),
                serde_json::json!("pause"),
                serde_json::json!(true)
            ]
        );
        assert_eq!(mutations[1][0], "loadfile");
        assert_eq!(mutations[2][0], "seek");
        assert_eq!(mutations[3][1], "volume");
        assert_eq!(mutations[4][1], "pause");
        no_overlap(dir.path());
    }
}
#[cfg(unix)]
#[test]
fn preparation_failures_retire_candidate_before_explicit_retry() {
    for mode in ["reject", "mismatch", "noarrival", "wrongvolume"] {
        let dir = tempfile::tempdir().unwrap();
        with_session(|session| {
            let request = submit(
                session,
                fake(dir.path(), mode),
                1,
                &dir.path().join("owned.wav"),
                if mode == "noarrival" {
                    Duration::from_millis(300)
                } else {
                    WAIT
                },
            );
            let failure = request.wait(WAIT).unwrap().result;
            match mode {
                "reject" => assert!(matches!(
                    failure,
                    Err(HandoffFailure::Engine(Error::Rejected))
                )),
                "mismatch" | "wrongvolume" => assert!(matches!(
                    failure,
                    Err(HandoffFailure::Engine(Error::Protocol))
                )),
                "noarrival" => assert!(
                    matches!(failure, Err(HandoffFailure::Deadline)),
                    "failure: {:?}",
                    failure.as_ref().err()
                ),
                _ => unreachable!(),
            }
            let next = submit(
                session,
                fake(dir.path(), "recovery"),
                2,
                &dir.path().join("owned.wav"),
                WAIT,
            );
            assert!(next.wait(WAIT).unwrap().result.is_ok());
            assert!(matches!(
                session.replace(dir.path().join("unused"), 1, AudioOutput::Null),
                Err(HandoffFailure::Stale)
            ));
        });
        no_overlap(dir.path());
        // Recovery checks all previous PIDs before binding; forced cleanup need
        // not leave a graceful exited event.
    }
}
#[cfg(unix)]
#[test]
fn cancellation_drop_pressure_and_shutdown_during_preparation() {
    for action in ["cancel", "drop", "shutdown"] {
        let dir = tempfile::tempdir().unwrap();
        with_session(|session| {
            let request = submit(
                session,
                fake(dir.path(), "ready"),
                1,
                &dir.path().join("owned.wav"),
                WAIT,
            );
            gate(&dir.path().join("position-ready"));
            let queued = submit(
                session,
                fake(dir.path(), "queued"),
                2,
                &dir.path().join("owned.wav"),
                WAIT,
            );
            assert!(matches!(
                session.replace(dir.path().join("unused"), 3, AudioOutput::Null),
                Err(HandoffFailure::QueueFull)
            ));
            queued.cancel();
            let start = Instant::now();
            match action {
                "drop" => drop(request),
                "shutdown" => {
                    session.shutdown();
                    assert!(matches!(
                        request.wait(WAIT).unwrap().result,
                        Err(HandoffFailure::Stopped)
                    ));
                }
                _ => {
                    request.cancel();
                    assert!(matches!(
                        request.wait(WAIT).unwrap().result,
                        Err(HandoffFailure::Cancelled)
                    ));
                }
            }
            assert!(start.elapsed() < Duration::from_secs(2));
            assert!(queued.wait(WAIT).unwrap().result.is_err());
            assert!(!dir.path().join("pid-queued").exists());
        });
        no_overlap(dir.path());
    }
}
#[cfg(unix)]
#[test]
fn invalid_preparation_does_not_consume_generation() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let binary = fake(dir.path(), "valid");
        for (source, position, volume) in [
            (PathBuf::new(), 1.0, 10.0),
            (PathBuf::from("bad\0path"), 1.0, 10.0),
            (dir.path().join("owned.wav"), f64::NAN, 10.0),
            (dir.path().join("owned.wav"), -1.0, 10.0),
            (dir.path().join("owned.wav"), 1.0, f64::INFINITY),
            (dir.path().join("owned.wav"), 1.0, 101.0),
        ] {
            assert!(matches!(
                session.replace_prepared(
                    binary.clone(),
                    0,
                    AudioOutput::Null,
                    LocalPreparation {
                        source,
                        position_seconds: position,
                        paused: true,
                        volume_percent: volume
                    },
                    WAIT
                ),
                Err(HandoffFailure::Engine(Error::InvalidInput))
            ));
        }
        assert!(matches!(
            session.replace_prepared(
                binary.clone(),
                0,
                AudioOutput::Null,
                preparation(&dir.path().join("owned.wav"), true),
                Duration::ZERO
            ),
            Err(HandoffFailure::Deadline)
        ));
        assert!(
            submit(session, binary, 0, &dir.path().join("owned.wav"), WAIT)
                .wait(WAIT)
                .unwrap()
                .result
                .is_ok()
        );
    });
}
#[cfg(unix)]
#[test]
fn expired_queued_budget_preserves_the_completed_active_endpoint() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let active = submit(
            session,
            fake(dir.path(), "ready"),
            1,
            &dir.path().join("owned.wav"),
            WAIT,
        );
        gate(&dir.path().join("position-ready"));
        let expired = submit(
            session,
            fake(dir.path(), "never"),
            2,
            &dir.path().join("owned.wav"),
            Duration::from_millis(1),
        );
        // The deadline is the condition being tested; this wait never controls peer progress.
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(dir.path().join("release-ready-ready"), []).unwrap();
        let client = active.wait(WAIT).unwrap().result.unwrap();
        assert!(matches!(
            expired.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Deadline)
        ));
        assert!(matches!(
            command(&client, 1, ControlCommand::PlaybackStatus),
            ControlValue::PlaybackStatus(NativePlaybackStatus::Paused)
        ));
        assert!(!dir.path().join("pid-never").exists());
    });
    no_overlap(dir.path());
}
fn real_restore(binaries: &[PathBuf]) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("owned.wav");
    write_tone(&source, 8, 750).unwrap();
    with_session(|session| {
        let mut old: Option<ControlClient> = None;
        for (index, binary) in binaries.iter().enumerate() {
            let generation = index as u64 + 1;
            let paused = index % 2 == 0;
            let expected_position = 1.0 + index as f64 * 0.25;
            let expected_volume = 25.0 + index as f64 * 10.0;
            let mut prep = preparation(&source, paused);
            prep.position_seconds = expected_position;
            prep.volume_percent = expected_volume;
            let started = Instant::now();
            let client = session
                .replace_prepared(binary.clone(), generation, AudioOutput::Null, prep, WAIT)
                .unwrap()
                .wait(WAIT)
                .unwrap()
                .result
                .unwrap();
            if let Some(old) = old {
                assert!(matches!(
                    old.submit(generation - 1, 1, WAIT, ControlCommand::Position),
                    Err(ControlFailure::Stopped)
                ));
            }
            let expected_status = if paused {
                NativePlaybackStatus::Paused
            } else {
                NativePlaybackStatus::Playing
            };
            assert!(
                matches!(command(&client, generation, ControlCommand::PlaybackStatus), ControlValue::PlaybackStatus(s) if s == expected_status)
            );
            let observed = command(&client, generation, ControlCommand::Position);
            let elapsed = started.elapsed().as_secs_f64();
            assert!(matches!(observed, ControlValue::Position(Some(position)) if
                if paused { (position - expected_position).abs() <= 0.1 }
                else { position >= expected_position - 0.1 && position <= expected_position + elapsed + 0.1 }
            ));
            assert!(
                matches!(command(&client, generation, ControlCommand::VolumePercent), ControlValue::VolumePercent(v) if (v - expected_volume).abs() < 0.01)
            );
            old = Some(client);
        }
    });
}
fn mpv() -> PathBuf {
    std::env::var_os("P01_MPV")
        .unwrap_or_else(|| "mpv".into())
        .into()
}
#[test]
#[ignore = "requires external mpv; owned local tone/null output, no audio device certification"]
fn real_mpv_prepared_replacement_restore() {
    real_restore(&[mpv(), mpv(), mpv(), mpv()]);
}
#[cfg(all(feature = "gst-probe", target_os = "linux"))]
#[test]
#[ignore = "requires external mpv and GStreamer; null output only"]
fn real_mpv_gstreamer_prepared_replacement_restore() {
    real_restore(&[
        mpv(),
        PathBuf::from(env!("CARGO_BIN_EXE_p01-gst-peer")),
        mpv(),
    ]);
}
#[cfg(unix)]
#[test]
fn replacement_retires_old_clones_before_candidate_is_ready() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let old = submit(
            session,
            fake(dir.path(), "old"),
            1,
            &dir.path().join("owned.wav"),
            WAIT,
        )
        .wait(WAIT)
        .unwrap()
        .result
        .unwrap();
        let clone = old.clone();
        let request = submit(
            session,
            fake(dir.path(), "ready"),
            2,
            &dir.path().join("owned.wav"),
            WAIT,
        );
        gate(&dir.path().join("position-ready"));
        for retired in [&old, &clone] {
            assert!(matches!(
                retired.submit(1, 1, WAIT, ControlCommand::Position),
                Err(ControlFailure::Stopped)
            ));
        }
        assert!(request.try_reply().unwrap().is_none());
        request.cancel();
        assert!(matches!(
            request.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Cancelled)
        ));
        assert!(matches!(
            old.submit(1, 2, WAIT, ControlCommand::Pause(false)),
            Err(ControlFailure::Stopped)
        ));
    });
    let events = trace(dir.path());
    assert!(
        events
            .iter()
            .position(|e| e["name"] == "old" && e["event"] == "exited")
            .unwrap()
            < events
                .iter()
                .position(|e| e["name"] == "ready" && e["event"] == "started")
                .unwrap()
    );
    no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn target_beyond_duration_rejects_without_sending_seek() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let mut prep = preparation(&dir.path().join("owned.wav"), true);
        prep.position_seconds = 30.0;
        let request = session
            .replace_prepared(fake(dir.path(), "beyond"), 1, AudioOutput::Null, prep, WAIT)
            .unwrap();
        assert!(matches!(
            request.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Engine(Error::Unsupported))
        ));
        assert!(
            submit(
                session,
                fake(dir.path(), "recovery"),
                2,
                &dir.path().join("owned.wav"),
                WAIT
            )
            .wait(WAIT)
            .unwrap()
            .result
            .is_ok()
        );
    });
    assert!(
        !trace(dir.path())
            .iter()
            .any(|e| e["name"] == "beyond" && e["command"][0] == "seek")
    );
    no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn zero_position_is_restored_with_one_explicit_seek() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let binary = fake(dir.path(), "zero");
        let mut prep = preparation(&dir.path().join("owned.wav"), true);
        prep.position_seconds = 0.0;
        let client = session
            .replace_prepared(binary, 1, AudioOutput::Null, prep, WAIT)
            .unwrap()
            .wait(WAIT)
            .unwrap()
            .result
            .unwrap();
        assert!(matches!(
            command(&client, 1, ControlCommand::Position),
            ControlValue::Position(Some(position)) if position == 0.0
        ));
        assert!(matches!(
            command(&client, 1, ControlCommand::PlaybackStatus),
            ControlValue::PlaybackStatus(NativePlaybackStatus::Paused)
        ));
        assert!(matches!(
            command(&client, 1, ControlCommand::VolumePercent),
            ControlValue::VolumePercent(volume) if volume == 37.0
        ));
    });
    let events = trace(dir.path());
    let seeks: Vec<_> = events
        .iter()
        .filter(|event| event["command"][0] == "seek")
        .collect();
    assert_eq!(seeks.len(), 1);
    assert_eq!(seeks[0]["command"][1], 0.0);
    no_overlap(dir.path());
}
