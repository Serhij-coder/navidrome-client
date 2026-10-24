#![cfg(target_os = "linux")]
use feishin_native_spike::*;
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::ObjectPath,
};
const NAME: &str = "org.mpris.MediaPlayer2.FeishinP01.OwnedTest";
const PATH: &str = "/org/mpris/MediaPlayer2";
const WAIT: Duration = Duration::from_secs(8);
struct Bus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}
impl Bus {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--print-address=1",
                &format!("--address=unix:path={}", dir.path().join("bus").display()),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("private dbus-daemon required for feature tests");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(address.starts_with("unix:path="));
        Self {
            child,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }
    fn connect(&self) -> Connection {
        zbus::blocking::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .unwrap()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn fake(dir: &Path, mode: &str) -> PathBuf {
    let path = dir.join(mode);
    std::os::unix::fs::symlink(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/native-spike/fake_mpv.py"
        ),
        &path,
    )
    .unwrap();
    path
}
fn ready(session: &ControlSession, source: PathBuf) {
    assert!(
        session
            .client()
            .submit(
                5,
                0,
                Duration::from_secs(3),
                ControlCommand::LoadLocal(source)
            )
            .unwrap()
            .wait(WAIT)
            .unwrap()
            .result
            .is_ok()
    );
}
// Real process preparation observes readiness, rather than treating LoadLocal's
// acknowledgement as loaded media. Pause first so the fixture cannot run ahead.
fn prepare_real_source(session: &ControlSession, source: PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let client = session.client();
    for (id, command) in [
        (1, ControlCommand::Pause(true)),
        (2, ControlCommand::LoadLocal(source)),
    ] {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let reply = client
            .submit(5, id, remaining, command)
            .unwrap()
            .wait(remaining)
            .unwrap();
        assert!(
            matches!(reply.result, Ok(ControlValue::Acknowledged)),
            "preparation request {id}: {:?}",
            reply.result
        );
    }
    let mut id = 2;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("source readiness deadline");
        id += 1;
        let result = client
            .submit(5, id, remaining, ControlCommand::Position)
            .unwrap()
            .wait(remaining)
            .unwrap()
            .result;
        match result {
            Ok(ControlValue::Position(Some(position))) if position.abs() <= 0.1 => break,
            Ok(ControlValue::Position(_)) => {}
            other => panic!("source readiness request {id}: {other:?}"),
        }
        std::thread::sleep(
            Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
fn player(connection: &Connection) -> Proxy<'_> {
    zbus::blocking::proxy::Builder::new(connection)
        .destination(NAME)
        .unwrap()
        .path(PATH)
        .unwrap()
        .interface("org.mpris.MediaPlayer2.Player")
        .unwrap()
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .unwrap()
}
#[test]
fn checked_units() {
    assert_eq!(micros_to_seconds(1_250_000).unwrap(), 1.25);
    assert_eq!(seconds_to_micros(1.25).unwrap(), 1_250_000);
    assert!(micros_to_seconds(-1).is_err());
    for value in [-1.0, f64::NAN, f64::INFINITY, i64::MAX as f64 / 1e6] {
        assert!(seconds_to_micros(value).is_err());
    }
    assert_eq!(seconds_to_micros(0.000_001).unwrap(), 1);
}
#[test]
fn private_bus_routes_observed_properties_rejects_mutations_and_tears_down() {
    exercise_bus(false);
}
#[test]
#[ignore = "requires real mpv on PATH; private bus and owned tone/null output only"]
fn real_private_bus_mpv() {
    exercise_bus(true);
}
fn exercise_bus(real: bool) {
    let bus = Bus::start();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("owned.wav");
    write_tone(&source, 10, 440).unwrap();
    let session = ControlSession::start(
        if real {
            "mpv".into()
        } else {
            fake(dir.path(), "mpris-stateful")
        },
        5,
        AudioOutput::Null,
    )
    .unwrap();
    run_with_cleanup(&session, || {
        if real {
            prepare_real_source(&session, source);
        } else {
            ready(&session, source);
        }
        for address in [
            "",
            "tcp:host=localhost,port=1234",
            "unix:path=/tmp/x;unix:path=/tmp/y",
        ] {
            assert!(matches!(
                MprisProbe::on_owned_bus(address, NAME, session.client(), 5),
                Err(Error::InvalidInput)
            ));
        }
        assert!(matches!(
            MprisProbe::on_owned_bus(
                &bus.address,
                "org.mpris.MediaPlayer2.Feishin",
                session.client(),
                5
            ),
            Err(Error::InvalidInput)
        ));
        let service = MprisProbe::on_owned_bus(&bus.address, NAME, session.client(), 5).unwrap();
        let connection = bus.connect();
        let player = player(&connection);
        let paused = player.call::<_, _, ()>("Pause", &());
        if paused.is_err() {
            let typed = session
                .client()
                .submit(
                    5,
                    999,
                    Duration::from_secs(2),
                    ControlCommand::PlaybackStatus,
                )
                .unwrap()
                .wait(WAIT)
                .unwrap();
            panic!(
                "initial MPRIS Pause failed; typed status: {:?}",
                typed.result
            );
        }
        assert_eq!(
            player.get_property::<String>("PlaybackStatus").unwrap(),
            "Paused"
        );
        let track = ObjectPath::try_from(MPRIS_TRACK_PATH).unwrap();
        player
            .call::<_, _, ()>("SetPosition", &(track, 1_250_000_i64))
            .unwrap();
        // mpv seek acknowledgement precedes observed arrival. Poll the actual property.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let position = player.get_property::<i64>("Position").unwrap();
            if (position - 1_250_000).abs() < 100_000 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "observed position did not arrive"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        player.call::<_, _, ()>("Seek", &(-250_000_i64,)).unwrap();
        if !real {
            assert_eq!(player.get_property::<i64>("Position").unwrap(), 1_000_000);
        }
        player.call::<_, _, ()>("Play", &()).unwrap();
        assert_eq!(
            player.get_property::<String>("PlaybackStatus").unwrap(),
            "Playing"
        );
        player.call::<_, _, ()>("PlayPause", &()).unwrap();
        assert_eq!(
            player.get_property::<String>("PlaybackStatus").unwrap(),
            "Paused"
        );
        for (track, position) in [
            ("/stale", 0_i64),
            (MPRIS_TRACK_PATH, -1),
            (MPRIS_TRACK_PATH, 11_000_000),
        ] {
            let before = player.get_property::<i64>("Position").unwrap();
            player
                .call::<_, _, ()>(
                    "SetPosition",
                    &(ObjectPath::try_from(track).unwrap(), position),
                )
                .unwrap();
            assert_eq!(player.get_property::<i64>("Position").unwrap(), before);
        }
        player.set_property("Volume", 0.25_f64).unwrap();
        assert_eq!(player.get_property::<f64>("Volume").unwrap(), 0.25);
        player.set_property("Volume", -1.0_f64).unwrap();
        assert_eq!(player.get_property::<f64>("Volume").unwrap(), 0.0);
        player.set_property("Volume", 2.0_f64).unwrap();
        assert_eq!(player.get_property::<f64>("Volume").unwrap(), 1.0);
        assert!(player.set_property("Volume", f64::NAN).is_err());
        player.set_property("Rate", 1.0_f64).unwrap();
        assert!(player.set_property("Rate", 2.0_f64).is_err());
        for method in ["Next", "Previous", "Stop"] {
            assert!(player.call::<_, _, ()>(method, &()).is_err());
        }
        assert!(
            player
                .call::<_, _, ()>("OpenUri", &("file:///unowned",))
                .is_err()
        );
        assert!(!player.get_property::<bool>("CanGoNext").unwrap());
        assert!(player.get_property::<bool>("CanSeek").unwrap());
        let metadata = player
            .get_property::<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>(
                "Metadata",
            )
            .unwrap();
        assert_eq!(metadata.len(), 1);
        if real {
            let result = player.call::<_, _, ()>(
                "SetPosition",
                &(
                    ObjectPath::try_from(MPRIS_TRACK_PATH).unwrap(),
                    9_000_000_i64,
                ),
            );
            if result.is_err() {
                for (id, command) in [
                    (997, ControlCommand::Position),
                    (998, ControlCommand::Paused),
                    (999, ControlCommand::PlaybackStatus),
                ] {
                    let reply = session
                        .client()
                        .submit(5, id, Duration::from_secs(2), command)
                        .unwrap()
                        .wait(WAIT)
                        .unwrap();
                    eprintln!("EOF setup diagnostic request {id}: {:?}", reply.result);
                }
            }
            result.unwrap();
            player.call::<_, _, ()>("Play", &()).unwrap();
            let deadline = Instant::now() + WAIT;
            loop {
                if player
                    .get_property::<String>("PlaybackStatus")
                    .is_ok_and(|status| status == "Stopped")
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "natural EOF did not report Stopped"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        session.shutdown();
        session.wait_shutdown(WAIT).unwrap();
        assert!(player.call::<_, _, ()>("Play", &()).is_err());
        drop(service);
        let daemon = zbus::blocking::fdo::DBusProxy::new(&connection).unwrap();
        assert!(!daemon.name_has_owner(NAME.try_into().unwrap()).unwrap());
    });
}
// Test/CLI cleanup is deliberately blocking, unlike production ControlSession Drop.
fn run_with_cleanup(session: &ControlSession, run: impl FnOnce()) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    if let Err(panic) = outcome {
        session.shutdown();
        let cleanup = session.wait_shutdown(WAIT);
        if cleanup.is_err() && !matches!(cleanup, Err(ControlFailure::Stopped)) {
            eprintln!("probe cleanup failed: {cleanup:?}");
        }
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn deadline_admission_and_engine_death_are_explicit_errors() {
    let bus = Bus::start();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("owned.wav");
    write_tone(&source, 1, 440).unwrap();
    let session =
        ControlSession::start(fake(dir.path(), "slow-position"), 5, AudioOutput::Null).unwrap();
    ready(&session, source);
    let service = MprisProbe::on_owned_bus(&bus.address, NAME, session.client(), 5).unwrap();
    let connection = bus.connect();
    let started = Instant::now();
    let calls: Vec<_> = (0..20)
        .map(|_| {
            let connection = connection.clone();
            std::thread::spawn(move || {
                let started = Instant::now();
                let error = player(&connection)
                    .get_property::<i64>("Position")
                    .unwrap_err();
                (started.elapsed(), error.to_string())
            })
        })
        .collect();
    let results: Vec<_> = calls.into_iter().map(|call| call.join().unwrap()).collect();
    assert!(
        results
            .iter()
            .any(|(elapsed, error)| *elapsed < Duration::from_secs(1)
                && error.contains("LimitsExceeded")),
        "pending admission must reject pressure promptly: {results:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    session.wait_shutdown(WAIT).unwrap();
    assert!(player(&connection).call::<_, _, ()>("Pause", &()).is_err());
    drop(service);
}

#[test]
fn service_teardown_rejects_inflight_request_without_waiting_on_engine() {
    let bus = Bus::start();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("owned.wav");
    write_tone(&source, 1, 440).unwrap();
    let session =
        ControlSession::start(fake(dir.path(), "slow-position"), 5, AudioOutput::Null).unwrap();
    ready(&session, source);
    let service = MprisProbe::on_owned_bus(&bus.address, NAME, session.client(), 5).unwrap();
    let connection = bus.connect();
    let call = std::thread::spawn(move || player(&connection).get_property::<i64>("Position"));
    std::thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    drop(service);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(call.join().unwrap().is_err());
    session.shutdown();
    session.wait_shutdown(WAIT).unwrap();
}

#[test]
fn seek_signal_follows_observed_arrival_and_timeout_emits_no_signal() {
    for mode in ["delayed-position", "normal"] {
        let bus = Bus::start();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("owned.wav");
        write_tone(&source, 1, 440).unwrap();
        let session = ControlSession::start(fake(dir.path(), mode), 5, AudioOutput::Null).unwrap();
        ready(&session, source);
        let service = MprisProbe::on_owned_bus(&bus.address, NAME, session.client(), 5).unwrap();
        let connection = bus.connect();
        let listener_connection = bus.connect();
        let listener = player(&listener_connection);
        let mut signals = listener.receive_signal("Seeked").unwrap();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let (ready_send, ready_receive) = std::sync::mpsc::sync_channel(1);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                ready_send.send(()).unwrap();
                if let Some(signal) = signals.next() {
                    send.send(signal.body().deserialize::<(i64,)>().unwrap().0)
                        .unwrap();
                }
            });
            ready_receive.recv().unwrap();
            let started = Instant::now();
            let result = player(&connection).call::<_, _, ()>(
                "SetPosition",
                &(
                    ObjectPath::try_from(MPRIS_TRACK_PATH).unwrap(),
                    1_250_000_i64,
                ),
            );
            if mode == "delayed-position" {
                result.unwrap();
                assert!(
                    started.elapsed() >= Duration::from_millis(30),
                    "must not signal initial seek acknowledgement"
                );
                assert_eq!(
                    receive.recv_timeout(Duration::from_secs(1)).unwrap(),
                    1_250_000
                );
                assert_eq!(
                    player(&connection).get_property::<i64>("Position").unwrap(),
                    1_250_000
                );
            } else {
                assert!(result.is_err());
                assert!(started.elapsed() < Duration::from_secs(3));
                assert!(matches!(
                    receive.try_recv(),
                    Err(std::sync::mpsc::TryRecvError::Empty)
                ));
            }
            drop(service);
            listener_connection.clone().close().unwrap();
        });
        session.shutdown();
        session.wait_shutdown(WAIT).unwrap();
    }
}

#[test]
fn private_bus_status_reports_idle_and_loading_honestly() {
    for mode in ["mpris-idle", "mpris-loading"] {
        let bus = Bus::start();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("owned.wav");
        write_tone(&source, 1, 440).unwrap();
        let session = ControlSession::start(fake(dir.path(), mode), 5, AudioOutput::Null).unwrap();
        ready(&session, source);
        let service = MprisProbe::on_owned_bus(&bus.address, NAME, session.client(), 5).unwrap();
        let connection = bus.connect();
        let status = player(&connection).get_property::<String>("PlaybackStatus");
        if mode == "mpris-idle" {
            assert_eq!(status.unwrap(), "Stopped");
        } else {
            assert!(status.is_err());
        }
        drop(service);
        session.shutdown();
        session.wait_shutdown(WAIT).unwrap();
    }
}

#[test]
fn observed_preparation_and_panic_cleanup_stop_owned_worker() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("owned.wav");
    write_tone(&source, 1, 440).unwrap();
    let session =
        ControlSession::start(fake(dir.path(), "delayed-position"), 5, AudioOutput::Null).unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_with_cleanup(&session, || {
            prepare_real_source(&session, source);
            panic!("intentional probe assertion regression");
        });
    }));
    assert!(outcome.is_err());
    assert!(matches!(
        session
            .client()
            .submit(5, 100, Duration::from_secs(1), ControlCommand::Pause(false)),
        Err(ControlFailure::Stopped)
    ));
    assert!(
        matches!(session.wait_shutdown(WAIT), Err(ControlFailure::Stopped)),
        "cleanup must have consumed worker completion after mpv teardown"
    );
}
