#![cfg(any(unix, windows))]
use feishin_native_spike::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
const WAIT: Duration = Duration::from_secs(8);
#[cfg(unix)]
fn fake(directory: &Path, mode: &str) -> std::path::PathBuf {
    let executable = directory.join(mode);
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
fn command(
    client: &ControlClient,
    generation: u64,
    id: u64,
    command: ControlCommand,
) -> ControlReply {
    client
        .submit(generation, id, Duration::from_secs(3), command)
        .unwrap()
        .wait(WAIT)
        .unwrap()
}
#[cfg(unix)]
#[test]
fn typed_routes_validation_stale_and_missing_process() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("owned.wav");
    write_tone(&tone, 2, 440).unwrap();
    let session = ControlSession::start(fake(dir.path(), "normal"), 7, AudioOutput::Null).unwrap();
    let client = session.client();
    let expired = client
        .submit(7, 0, Duration::from_nanos(1), ControlCommand::Pause(false))
        .unwrap();
    assert!(matches!(
        expired.wait(WAIT).unwrap().result,
        Err(ControlFailure::Deadline)
    ));
    assert!(matches!(
        command(&client, 6, 1, ControlCommand::Pause(true)).result,
        Err(ControlFailure::Stale)
    ));
    assert!(matches!(
        client.submit(7, 2, WAIT, ControlCommand::SeekSeconds(f64::NAN)),
        Err(ControlFailure::Engine(Error::InvalidInput))
    ));
    for path in ["".to_owned(), "a".repeat(4097), "invalid\0path".to_owned()] {
        assert!(matches!(
            client.submit(7, 2, WAIT, ControlCommand::LoadLocal(path.into())),
            Err(ControlFailure::Engine(Error::InvalidInput))
        ));
    }
    assert!(matches!(
        command(&client, 7, 3, ControlCommand::LoadLocal(tone)).result,
        Ok(ControlValue::Acknowledged)
    ));
    for (id, cmd) in [
        (4, ControlCommand::Pause(true)),
        (5, ControlCommand::SeekSeconds(0.5)),
        (6, ControlCommand::Pause(false)),
    ] {
        let reply = command(&client, 7, id, cmd);
        assert_eq!((reply.generation, reply.request_id), (7, id));
        assert!(matches!(reply.result, Ok(ControlValue::Acknowledged)));
    }
    assert!(matches!(
        command(&client, 7, 7, ControlCommand::Position).result,
        Ok(ControlValue::Position(Some(_)))
    ));
    session.shutdown();
    session.wait_shutdown(WAIT).unwrap();
    assert!(matches!(
        client.submit(7, 8, WAIT, ControlCommand::Position),
        Err(ControlFailure::Stopped)
    ));
    let missing = ControlSession::start(dir.path().join("missing"), 9, AudioOutput::Null).unwrap();
    assert!(matches!(
        missing.wait_shutdown(WAIT),
        Err(ControlFailure::Engine(_))
    ));
}
#[cfg(unix)]
#[test]
fn bounded_ingress_and_shutdown_during_startup() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let session =
        ControlSession::start(fake(dir.path(), "slow-start"), 1, AudioOutput::Null).unwrap();
    let client = session.client();
    let requests: Vec<_> = (0..CONTROL_CAPACITY)
        .map(|id| {
            client
                .submit(1, id as u64, WAIT, ControlCommand::Position)
                .unwrap()
        })
        .collect();
    assert!(matches!(
        client.submit(1, 99, WAIT, ControlCommand::Position),
        Err(ControlFailure::QueueFull)
    ));
    session.shutdown();
    assert!(matches!(
        session.wait_shutdown(WAIT),
        Err(ControlFailure::Engine(Error::Deadline))
    ));
    assert!(started.elapsed() < WAIT);
    for request in requests {
        assert!(matches!(
            request.wait(WAIT).unwrap().result,
            Err(ControlFailure::Stopped)
        ));
    }
}
#[cfg(unix)]
#[test]
fn position_deadline_process_death_and_abandoned_requests() {
    let dir = tempfile::tempdir().unwrap();
    let session =
        ControlSession::start(fake(dir.path(), "slow-position"), 1, AudioOutput::Null).unwrap();
    let client = session.client();
    // Readiness barrier: startup completed before exercising the short IPC budget.
    assert!(
        command(&client, 1, 1, ControlCommand::Pause(true))
            .result
            .is_ok()
    );
    let request = client
        .submit(1, 2, Duration::from_millis(80), ControlCommand::Position)
        .unwrap();
    let expired = client
        .submit(1, 3, Duration::from_millis(1), ControlCommand::Pause(false))
        .unwrap();
    let cancelled = client
        .submit(1, 4, WAIT, ControlCommand::SeekSeconds(1.0))
        .unwrap();
    cancelled.cancel();
    drop(
        client
            .submit(1, 5, WAIT, ControlCommand::SeekSeconds(2.0))
            .unwrap(),
    );
    assert!(matches!(
        request.wait(WAIT).unwrap().result,
        Err(ControlFailure::Deadline)
    ));
    assert!(matches!(
        expired.wait(WAIT).unwrap().result,
        Err(ControlFailure::Stopped)
    ));
    assert!(matches!(
        cancelled.wait(WAIT).unwrap().result,
        Err(ControlFailure::Stopped)
    ));
    session.shutdown();
    session.wait_shutdown(WAIT).unwrap();
    let died = ControlSession::start(fake(dir.path(), "die"), 2, AudioOutput::Null).unwrap();
    assert!(matches!(
        command(&died.client(), 2, 1, ControlCommand::Pause(true)).result,
        Err(ControlFailure::Engine(_))
    ));
    died.wait_shutdown(WAIT).unwrap();
    assert!(matches!(
        died.client().submit(2, 2, WAIT, ControlCommand::Position),
        Err(ControlFailure::Stopped)
    ));
}
#[cfg(unix)]
#[test]
fn mutation_deadline_helpers_reject_before_ipc_without_invalidating_engine() {
    let dir = tempfile::tempdir().unwrap();
    let mut mpv = Mpv::start(&fake(dir.path(), "normal"), 1, AudioOutput::Null).unwrap();
    let expired = Instant::now() - Duration::from_secs(1);
    assert!(matches!(
        mpv.pause_until(true, expired),
        Err(Error::Deadline)
    ));
    assert!(matches!(mpv.seek_until(1.0, expired), Err(Error::Deadline)));
    assert!(matches!(
        mpv.load_local_until(&dir.path().join("missing"), false, expired),
        Err(Error::Deadline)
    ));
    assert!(mpv.accepts_generation(1));
    mpv.pause(false).unwrap();
    mpv.shutdown().unwrap();
}
#[cfg(unix)]
#[test]
fn mutation_ipc_uses_short_remaining_deadlines() {
    for kind in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let tone = dir.path().join("owned.wav");
        write_tone(&tone, 1, 440).unwrap();
        let mut mpv = Mpv::start(&fake(dir.path(), "slow-drip"), 1, AudioOutput::Null).unwrap();
        let start = Instant::now();
        let deadline = start + Duration::from_millis(80);
        let result = match kind {
            0 => mpv.pause_until(true, deadline),
            1 => mpv.seek_until(0.5, deadline),
            _ => mpv.load_local_until(&tone, false, deadline),
        };
        assert!(matches!(result, Err(Error::Deadline)));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!mpv.accepts_generation(1));
        mpv.shutdown().unwrap();
    }
}
// Test-only observed workflow. Raw LoadLocal/SeekSeconds replies remain ACKs.
fn routed_position(
    client: &ControlClient,
    generation: u64,
    request_id: &mut u64,
    expected: f64,
    deadline: Instant,
) -> Result<f64, ControlFailure> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(ControlFailure::Deadline)?;
        *request_id += 1;
        let reply = client
            .submit(generation, *request_id, remaining, ControlCommand::Position)?
            .wait(remaining)?;
        match reply.result? {
            ControlValue::Position(Some(position)) if (position - expected).abs() <= 0.1 => {
                return Ok(position);
            }
            ControlValue::Position(_) => {}
            _ => return Err(ControlFailure::Engine(Error::Protocol)),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(Duration::from_millis(5).min(remaining));
    }
}

fn routed_paused_seek(client: &ControlClient, generation: u64, tone: std::path::PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut id = 0;
    let mut issue = |cmd| {
        id += 1;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let reply = client
            .submit(generation, id, remaining, cmd)
            .unwrap()
            .wait(remaining)
            .unwrap();
        assert!(
            matches!(reply.result, Ok(ControlValue::Acknowledged)),
            "request {id}: {:?}",
            reply.result
        );
    };
    // Pause before loading so startup cannot consume the short fixture while
    // readiness is observed. Never retry a rejected mutation.
    issue(ControlCommand::Pause(true));
    issue(ControlCommand::LoadLocal(tone));
    routed_position(client, generation, &mut id, 0.0, deadline)
        .unwrap_or_else(|error| panic!("readiness request {id}: {error:?}"));
    id += 1;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let reply = client
        .submit(generation, id, remaining, ControlCommand::SeekSeconds(1.0))
        .unwrap()
        .wait(remaining)
        .unwrap();
    assert!(
        matches!(reply.result, Ok(ControlValue::Acknowledged)),
        "seek request {id}: {:?}",
        reply.result
    );
    routed_position(client, generation, &mut id, 1.0, deadline)
        .unwrap_or_else(|error| panic!("seek observation request {id}: {error:?}"));
    let reply = command(client, generation, id + 1, ControlCommand::Pause(false));
    assert!(
        matches!(reply.result, Ok(ControlValue::Acknowledged)),
        "resume request {}: {:?}",
        id + 1,
        reply.result
    );
}

// Only this blocking test harness waits during unwinding. Production Drop stays
// nonblocking; the worker must finish before the test process can exit.
fn with_cleanup(session: &ControlSession, probe: impl FnOnce()) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(probe));
    session.shutdown();
    let cleanup = session.wait_shutdown(WAIT);
    assert!(cleanup.is_ok(), "control cleanup: {cleanup:?}");
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

#[cfg(unix)]
#[test]
fn routed_seek_waits_for_readiness_and_observed_arrival() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("owned.wav");
    write_tone(&tone, 4, 440).unwrap();
    let session =
        ControlSession::start(fake(dir.path(), "delayed-position"), 3, AudioOutput::Null).unwrap();
    with_cleanup(&session, || routed_paused_seek(&session.client(), 3, tone));
}

#[cfg(unix)]
#[test]
fn routed_missing_readiness_expires_and_panic_cleanup_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let session = ControlSession::start(
        fake(dir.path(), "unavailable-position"),
        3,
        AudioOutput::Null,
    )
    .unwrap();
    let client = session.client();
    // Startup barrier leaves the observation budget independent of spawn timing.
    assert!(
        command(&client, 3, 1, ControlCommand::Pause(true))
            .result
            .is_ok()
    );
    let mut id = 1;
    let result = routed_position(
        &client,
        3,
        &mut id,
        0.0,
        Instant::now() + Duration::from_millis(40),
    );
    assert!(matches!(result, Err(ControlFailure::Deadline)));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_cleanup(&session, || panic!("intentional probe assertion"));
    }));
    assert!(outcome.is_err());
    assert!(matches!(
        client.submit(3, id + 1, WAIT, ControlCommand::Position),
        Err(ControlFailure::Stopped)
    ));
}

#[test]
#[ignore = "requires real mpv; owned synthetic tone/null output, no physical audio certification"]
fn real_mpv_control_routing() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("owned.wav");
    write_tone(&tone, 4, 440).unwrap();
    let session = ControlSession::start(Path::new("mpv").into(), 3, AudioOutput::Null).unwrap();
    with_cleanup(&session, || routed_paused_seek(&session.client(), 3, tone));
}

#[cfg(unix)]
#[test]
fn observed_status_distinguishes_idle_pause_and_loading_without_duplicate_state() {
    for (mode, expected) in [
        ("mpris-stateful", Some(NativePlaybackStatus::Playing)),
        ("mpris-idle", Some(NativePlaybackStatus::Stopped)),
        ("mpris-loading", None),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("owned.wav");
        write_tone(&source, 1, 440).unwrap();
        let session = ControlSession::start(fake(dir.path(), mode), 7, AudioOutput::Null).unwrap();
        let client = session.client();
        assert!(
            command(&client, 7, 1, ControlCommand::LoadLocal(source))
                .result
                .is_ok()
        );
        let status = command(&client, 7, 2, ControlCommand::PlaybackStatus).result;
        match expected {
            Some(expected) => assert!(
                matches!(status, Ok(ControlValue::PlaybackStatus(actual)) if actual == expected)
            ),
            None => assert!(matches!(
                status,
                Err(ControlFailure::Engine(Error::PropertyUnavailable))
            )),
        }
        assert!(
            command(&client, 7, 3, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        if expected.is_some() {
            let expected = if mode == "mpris-idle" {
                NativePlaybackStatus::Stopped
            } else {
                NativePlaybackStatus::Paused
            };
            assert!(
                matches!(command(&client, 7, 4, ControlCommand::PlaybackStatus).result, Ok(ControlValue::PlaybackStatus(actual)) if actual == expected)
            );
        } else {
            assert!(
                matches!(
                    command(&client, 7, 4, ControlCommand::Paused).result,
                    Ok(ControlValue::Paused(true))
                ),
                "normal loading must not invalidate IPC"
            );
        }
        session.shutdown();
        session.wait_shutdown(WAIT).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn invalid_successful_properties_stop_owner_but_unavailable_loading_does_not() {
    for (mode, query) in [
        ("mpris-invalid-idle", ControlCommand::PlaybackStatus),
        (
            "mpris-invalid-duration",
            ControlCommand::SeekObservedSeconds(0.5),
        ),
        ("mpris-invalid-volume", ControlCommand::VolumePercent),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let session = ControlSession::start(fake(dir.path(), mode), 7, AudioOutput::Null).unwrap();
        let client = session.client();
        assert!(matches!(
            command(&client, 7, 1, query).result,
            Err(ControlFailure::Engine(Error::Protocol))
        ));
        session.wait_shutdown(WAIT).unwrap();
        assert!(matches!(
            client.submit(7, 2, WAIT, ControlCommand::Pause(false)),
            Err(ControlFailure::Stopped)
        ));
    }
    for (mode, query) in [
        ("mpris-unavailable-idle", ControlCommand::PlaybackStatus),
        (
            "mpris-unavailable-duration",
            ControlCommand::SeekObservedSeconds(0.5),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let session = ControlSession::start(fake(dir.path(), mode), 7, AudioOutput::Null).unwrap();
        let client = session.client();
        assert!(matches!(
            command(&client, 7, 1, query).result,
            Err(ControlFailure::Engine(Error::PropertyUnavailable))
        ));
        assert!(
            command(&client, 7, 2, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        session.shutdown();
        session.wait_shutdown(WAIT).unwrap();
    }
}
