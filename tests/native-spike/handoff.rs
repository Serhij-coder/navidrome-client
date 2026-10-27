#![cfg(any(unix, windows))]
use feishin_native_spike::*;
#[cfg(unix)]
use std::path::Path;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(10);

// Cleanup remains explicit even when an assertion panics. Drop alone deliberately
// does not block the UI and therefore cannot certify process cleanup in a test.
fn with_session(check: impl FnOnce(&HandoffSession)) -> Result<(), HandoffFailure> {
    let session = HandoffSession::start().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&session)));
    session.shutdown();
    let cleanup = session.wait_shutdown(WAIT);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
    cleanup
}

fn endpoint(session: &HandoffSession, binary: PathBuf, generation: u64) -> ControlClient {
    let reply = session
        .replace(binary, generation, AudioOutput::Null)
        .unwrap()
        .wait(WAIT)
        .unwrap();
    assert_eq!(reply.generation, generation);
    reply.result.unwrap()
}

fn command(client: &ControlClient, generation: u64, id: u64, cmd: ControlCommand) -> ControlReply {
    client
        .submit(generation, id, Duration::from_secs(3), cmd)
        .unwrap()
        .wait(WAIT)
        .unwrap()
}

#[cfg(unix)]
fn fake(directory: &Path, mode: &str) -> PathBuf {
    let executable = directory.join(mode);
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/handoff_peer.py"
        ),
        &executable,
    )
    .unwrap();
    executable
}

#[cfg(unix)]
fn wait_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !path.exists() {
        assert!(Instant::now() < deadline, "fixture gate was not reached");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(unix)]
fn wait_retired(client: &ControlClient, generation: u64) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match client.submit(generation, 90, WAIT, ControlCommand::Position) {
            Err(ControlFailure::Stopped) => return,
            Err(ControlFailure::QueueFull) | Ok(_) => {}
            other => panic!("unexpected retirement result: {}", other.is_err()),
        }
        assert!(Instant::now() < deadline, "old endpoint was not retired");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(unix)]
fn events(directory: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(directory.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[cfg(unix)]
fn assert_no_overlap(directory: &Path) {
    assert!(
        !events(directory)
            .iter()
            .any(|event| event["event"] == "overlap")
    );
}

#[cfg(unix)]
#[test]
fn replacement_retires_clones_rejects_old_generations_and_reaps_stalled_quit() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let old = endpoint(session, fake(dir.path(), "stalled-quit-1"), 1);
        let clone = old.clone();
        assert!(
            command(&old, 1, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        let start = Instant::now();
        let new = endpoint(session, fake(dir.path(), "normal-2"), 2);
        assert!(start.elapsed() < Duration::from_secs(3));
        for retired in [&old, &clone] {
            assert!(matches!(
                retired.submit(1, 2, WAIT, ControlCommand::Position),
                Err(ControlFailure::Stopped)
            ));
        }
        assert!(matches!(
            session.replace(dir.path().join("unused"), 1, AudioOutput::Null),
            Err(HandoffFailure::Stale)
        ));
        assert!(matches!(
            session.replace(dir.path().join("unused"), 2, AudioOutput::Null),
            Err(HandoffFailure::Stale)
        ));
        assert!(matches!(
            command(&new, 1, 3, ControlCommand::Position).result,
            Err(ControlFailure::Stale)
        ));
        assert!(
            command(&new, 2, 4, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
    })
    .unwrap();
    assert_no_overlap(dir.path());
    let trace = events(dir.path());
    let quit = trace
        .iter()
        .position(|event| event["name"] == "stalled-quit-1" && event["event"] == "quit")
        .unwrap();
    let start = trace
        .iter()
        .position(|event| event["name"] == "normal-2" && event["event"] == "started")
        .unwrap();
    assert!(quit < start);
}

#[cfg(unix)]
#[test]
fn queued_cancel_and_drop_consume_generations_without_starting_processes() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let old = endpoint(session, fake(dir.path(), "gate-start-1"), 1);
        wait_file(&dir.path().join("started-gate-start-1"));
        let replacement = session
            .replace(fake(dir.path(), "normal-2"), 2, AudioOutput::Null)
            .unwrap();
        wait_retired(&old, 1);
        let cancelled = session
            .replace(fake(dir.path(), "normal-3"), 3, AudioOutput::Null)
            .unwrap();
        cancelled.cancel();
        assert!(matches!(
            session.replace(dir.path().join("unused"), 4, AudioOutput::Null),
            Err(HandoffFailure::QueueFull)
        ));
        std::fs::write(dir.path().join("release-gate-start-1"), []).unwrap();
        assert!(replacement.wait(WAIT).unwrap().result.is_ok());
        assert!(matches!(
            cancelled.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Cancelled)
        ));
        assert!(matches!(
            session.replace(dir.path().join("unused"), 3, AudioOutput::Null),
            Err(HandoffFailure::Stale)
        ));
        let next = endpoint(session, fake(dir.path(), "gate-start-4"), 4);
        wait_file(&dir.path().join("started-gate-start-4"));
        let replacing = session
            .replace(fake(dir.path(), "normal-5"), 5, AudioOutput::Null)
            .unwrap();
        wait_retired(&next, 4);
        drop(
            session
                .replace(fake(dir.path(), "normal-6"), 6, AudioOutput::Null)
                .unwrap(),
        );
        std::fs::write(dir.path().join("release-gate-start-4"), []).unwrap();
        assert!(replacing.wait(WAIT).unwrap().result.is_ok());
        let last = endpoint(session, fake(dir.path(), "normal-7"), 7);
        assert!(
            command(&last, 7, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        assert!(matches!(
            session.replace(dir.path().join("unused"), 6, AudioOutput::Null),
            Err(HandoffFailure::Stale)
        ));
    })
    .unwrap();
    assert_no_overlap(dir.path());
    assert!(!dir.path().join("started-normal-3").exists());
    assert!(!dir.path().join("started-normal-6").exists());
}

#[cfg(unix)]
#[test]
fn shutdown_bypasses_full_ingress_and_never_activates_queued_replacements() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    with_session(|session| {
        let old = endpoint(session, fake(dir.path(), "gate-start-1"), 1);
        wait_file(&dir.path().join("started-gate-start-1"));
        let in_flight = session
            .replace(fake(dir.path(), "normal-2"), 2, AudioOutput::Null)
            .unwrap();
        wait_retired(&old, 1);
        let queued = session
            .replace(fake(dir.path(), "normal-3"), 3, AudioOutput::Null)
            .unwrap();
        session.shutdown();
        assert!(matches!(
            session.replace(dir.path().join("unused"), 4, AudioOutput::Null),
            Err(HandoffFailure::Stopped)
        ));
        std::fs::write(dir.path().join("release-gate-start-1"), []).unwrap();
        assert!(matches!(
            in_flight.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Stopped)
        ));
        assert!(matches!(
            queued.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Stopped)
        ));
    })
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!dir.path().join("started-normal-2").exists());
    assert!(!dir.path().join("started-normal-3").exists());
    assert_no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn missing_binary_fails_closed_and_fresh_supervisor_can_recover() {
    let dir = tempfile::tempdir().unwrap();
    let cleanup = with_session(|session| {
        let missing = endpoint(session, dir.path().join("missing-mpv"), 1);
        wait_retired(&missing, 1);
        let attempt = session
            .replace(fake(dir.path(), "never-start-2"), 2, AudioOutput::Null)
            .unwrap();
        assert!(attempt.wait(WAIT).unwrap().result.is_err());
        assert!(matches!(
            session.replace(dir.path().join("unused"), 3, AudioOutput::Null),
            Err(HandoffFailure::Stopped)
        ));
    });
    assert!(cleanup.is_err());
    assert!(!dir.path().join("started-never-start-2").exists());
    with_session(|session| {
        let client = endpoint(session, fake(dir.path(), "normal-recovery"), 1);
        assert!(
            command(&client, 1, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
    })
    .unwrap();
    assert_no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn dead_engine_is_reaped_before_replacement_and_old_control_stays_retired() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let old = endpoint(session, fake(dir.path(), "die-command-1"), 1);
        assert!(
            command(&old, 1, 1, ControlCommand::Pause(true))
                .result
                .is_err()
        );
        wait_retired(&old, 1);
        let new = endpoint(session, fake(dir.path(), "normal-2"), 2);
        assert!(
            command(&new, 2, 2, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        assert!(matches!(
            old.submit(1, 3, WAIT, ControlCommand::Position),
            Err(ControlFailure::Stopped)
        ));
    })
    .unwrap();
    assert_no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn cancellation_after_retirement_rejects_pending_old_work_and_stays_retired() {
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        let old = endpoint(session, fake(dir.path(), "gate-start-1"), 1);
        wait_file(&dir.path().join("started-gate-start-1"));
        let pending_old = old
            .submit(1, 1, WAIT, ControlCommand::Pause(false))
            .unwrap();
        let replacement = session
            .replace(fake(dir.path(), "never-start-2"), 2, AudioOutput::Null)
            .unwrap();
        wait_retired(&old, 1);
        replacement.cancel();
        assert!(replacement.try_reply().unwrap().is_none());
        assert!(matches!(
            replacement.wait(Duration::from_millis(1)),
            Err(HandoffFailure::Deadline)
        ));
        std::fs::write(dir.path().join("release-gate-start-1"), []).unwrap();
        assert!(matches!(
            replacement.wait(WAIT).unwrap().result,
            Err(HandoffFailure::Cancelled)
        ));
        assert!(matches!(
            pending_old.wait(WAIT).unwrap().result,
            Err(ControlFailure::Stopped)
        ));
        assert!(matches!(
            old.submit(1, 2, WAIT, ControlCommand::Position),
            Err(ControlFailure::Stopped)
        ));
        assert!(matches!(
            session.replace(dir.path().join("unused"), 2, AudioOutput::Null),
            Err(HandoffFailure::Stale)
        ));
        let new = endpoint(session, fake(dir.path(), "normal-3"), 3);
        assert!(
            command(&new, 3, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
    })
    .unwrap();
    assert!(!dir.path().join("started-never-start-2").exists());
    assert_no_overlap(dir.path());
}

#[cfg(unix)]
#[test]
fn invalid_inputs_do_not_consume_generation_and_max_generation_cannot_wrap() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let dir = tempfile::tempdir().unwrap();
    with_session(|session| {
        for binary in [
            PathBuf::new(),
            PathBuf::from("x".repeat(4097)),
            PathBuf::from("bad\0path"),
            PathBuf::from(OsString::from_vec(b"bad-\xff".to_vec())),
        ] {
            assert!(matches!(
                session.replace(binary, 0, AudioOutput::Null),
                Err(HandoffFailure::Engine(Error::InvalidInput))
            ));
        }
        let zero = endpoint(session, fake(dir.path(), "normal-zero"), 0);
        assert!(
            command(&zero, 0, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        let max = endpoint(session, fake(dir.path(), "normal-max"), u64::MAX);
        assert!(
            command(&max, u64::MAX, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        for generation in [0, 1, u64::MAX] {
            assert!(matches!(
                session.replace(dir.path().join("unused"), generation, AudioOutput::Null),
                Err(HandoffFailure::Stale)
            ));
        }
    })
    .unwrap();
    assert_no_overlap(dir.path());
}

fn wait_position(client: &ControlClient, generation: u64, expected: f64, next_id: &mut u64) {
    wait_position_until(
        client,
        generation,
        expected,
        next_id,
        Instant::now() + Duration::from_secs(3),
    )
    .unwrap();
}

fn wait_position_until(
    client: &ControlClient,
    generation: u64,
    expected: f64,
    next_id: &mut u64,
    deadline: Instant,
) -> Result<(), ControlFailure> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(ControlFailure::Deadline)?;
        *next_id += 1;
        let reply = client
            .submit(generation, *next_id, remaining, ControlCommand::Position)?
            .wait(remaining)?;
        match reply.result? {
            ControlValue::Position(Some(position)) if (position - expected).abs() <= 0.1 => {
                return Ok(());
            }
            ControlValue::Position(_) => {}
            _ => return Err(ControlFailure::Engine(Error::Protocol)),
        }
        std::thread::sleep(
            Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(unix)]
#[test]
fn position_observation_caps_slow_queries_by_remaining_shared_budget() {
    let directory = tempfile::tempdir().unwrap();
    with_session(|session| {
        let client = endpoint(session, fake(directory.path(), "stall-position-1"), 1);
        // Complete startup before measuring the observation's independent budget.
        assert!(
            command(&client, 1, 1, ControlCommand::Pause(true))
                .result
                .is_ok()
        );
        let start = Instant::now();
        let mut id = 1;
        assert!(matches!(
            wait_position_until(&client, 1, 0.0, &mut id, start + Duration::from_millis(80)),
            Err(ControlFailure::Deadline)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(id, 2);
    })
    .unwrap();
}

#[test]
#[ignore = "requires external mpv; owned local tone/null output, no audio device certification"]
fn real_repeated_handoff_paused_readiness_seek_and_retired_controls() {
    let directory = tempfile::tempdir().unwrap();
    let tone = directory.path().join("owned.wav");
    write_tone(&tone, 5, 750).unwrap();
    let binary = PathBuf::from(std::env::var_os("P01_MPV").unwrap_or_else(|| "mpv".into()));
    with_session(|session| {
        let mut previous: Option<ControlClient> = None;
        for generation in 1..=4 {
            let client = endpoint(session, binary.clone(), generation);
            if let Some(previous) = previous {
                assert!(matches!(
                    previous.submit(generation - 1, 99, WAIT, ControlCommand::Position),
                    Err(ControlFailure::Stopped)
                ));
            }
            assert!(
                command(&client, generation, 1, ControlCommand::Pause(true))
                    .result
                    .is_ok()
            );
            assert!(
                command(
                    &client,
                    generation,
                    2,
                    ControlCommand::LoadLocal(tone.clone())
                )
                .result
                .is_ok()
            );
            let mut id = 2;
            wait_position(&client, generation, 0.0, &mut id);
            id += 1;
            assert!(
                command(&client, generation, id, ControlCommand::SeekSeconds(1.0))
                    .result
                    .is_ok()
            );
            wait_position(&client, generation, 1.0, &mut id);
            previous = Some(client);
        }
    })
    .unwrap();
}
