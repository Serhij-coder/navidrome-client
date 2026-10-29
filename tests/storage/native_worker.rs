#![cfg(not(target_arch = "wasm32"))]
use feishin_storage::{
    native_worker::{NativeWorker, WorkerError},
    session::{Command, Response, SessionGeneration},
};
use std::{num::NonZeroU64, time::Duration};
#[test]
fn native_worker_roundtrip_and_unread_bound() {
    let dir = tempfile::tempdir().unwrap();
    for capacity in [0, 9] {
        assert!(matches!(
            NativeWorker::open(
                SessionGeneration(NonZeroU64::MIN),
                dir.path().into(),
                capacity,
            ),
            Err(WorkerError::InvalidCapacity)
        ));
    }
    let mut w = NativeWorker::open(
        SessionGeneration(NonZeroU64::new(1).unwrap()),
        dir.path().into(),
        1,
    )
    .unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    assert!(matches!(
        w.submit(Command::Load, Duration::from_secs(10)),
        Err(WorkerError::Full)
    ));
    assert!(matches!(receive(&mut w).result, Ok(Response::Load(_))));
    w.begin_shutdown();
    joined(&mut w).unwrap();
}

#[test]
fn queued_cancel_and_shutdown_with_unread_replies() {
    use feishin_storage::{AtomicStore, Error, session::SessionError};
    use std::sync::mpsc;
    struct Gate {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl AtomicStore for Gate {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    struct Empty;
    impl AtomicStore for Empty {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = NativeWorker::with_factory(
        SessionGeneration(NonZeroU64::new(2).unwrap()),
        2,
        move || {
            Ok((
                Gate {
                    entered: entered_tx,
                    release: release_rx,
                },
                Empty,
                Empty,
            ))
        },
    )
    .unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    assert!(!w.cancel(SessionGeneration(NonZeroU64::new(99).unwrap()), second));
    assert!(w.cancel(SessionGeneration(NonZeroU64::new(2).unwrap()), second));
    assert!(w.poll().unwrap().is_none());
    w.begin_shutdown();
    release_tx.send(()).unwrap();
    let first = receive(&mut w);
    let second = receive(&mut w);
    assert!(matches!(first.result, Ok(Response::Load(_))));
    assert!(matches!(second.result, Err(SessionError::Cancelled)));
    joined(&mut w).unwrap();
}
#[test]
fn factory_failure_is_reported_on_join() {
    use feishin_storage::{Error, NativeStore};
    let mut w = NativeWorker::with_factory::<NativeStore, NativeStore, NativeStore, _>(
        SessionGeneration(NonZeroU64::new(3).unwrap()),
        1,
        || Err(Error::Io),
    )
    .unwrap();
    assert!(matches!(
        joined(&mut w),
        Err(WorkerError::Factory(Error::Io))
    ));
}

fn receive(w: &mut NativeWorker) -> feishin_storage::session::Reply {
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(reply) = w.poll().unwrap() {
            return reply;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}
fn joined(w: &mut NativeWorker) -> Result<(), WorkerError> {
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = w.try_join() {
            return result;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}
fn execute(w: &mut NativeWorker, command: Command) -> Response {
    w.submit(command, Duration::from_secs(10)).unwrap();
    receive(w).result.ok().unwrap()
}
#[test]
fn native_import_adopt_undo_and_reopen() {
    use feishin_storage::session::OwnedInputs;
    let dir = tempfile::tempdir().unwrap();
    let generation = SessionGeneration(NonZeroU64::new(10).unwrap());
    let mut w = NativeWorker::open(generation, dir.path().into(), 1).unwrap();
    let input = OwnedInputs {
        settings: Some(
            br#"{"version":34,"general":{"language":"uk","followSystemTheme":true}}"#.to_vec(),
        ),
        ..Default::default()
    };
    let Response::StageReview { token, data, .. } = execute(&mut w, Command::PrepareStage(input))
    else {
        panic!()
    };
    assert_eq!(data.profile.preferences.language, "uk");
    assert!(matches!(
        execute(&mut w, Command::Apply(token)),
        Response::AppliedStage(_)
    ));
    let Response::ActiveReview { token, after, .. } = execute(&mut w, Command::PrepareAdoption)
    else {
        panic!()
    };
    assert_eq!(after.data.profile.preferences.language, "uk");
    assert!(matches!(
        execute(&mut w, Command::Apply(token)),
        Response::AppliedActive(_)
    ));
    let Response::ActiveReview { token, .. } = execute(&mut w, Command::PrepareRollback) else {
        panic!()
    };
    assert!(matches!(
        execute(&mut w, Command::Apply(token)),
        Response::AppliedActive(_)
    ));
    w.begin_shutdown();
    joined(&mut w).unwrap();
    let mut reopened = NativeWorker::open(
        SessionGeneration(NonZeroU64::new(11).unwrap()),
        dir.path().into(),
        1,
    )
    .unwrap();
    let Response::Load(loaded) = execute(&mut reopened, Command::Load) else {
        panic!()
    };
    assert_eq!(loaded.snapshot.revision, 2);
    assert_eq!(loaded.snapshot.data.profile.preferences.language, "en");
    reopened.begin_shutdown();
    joined(&mut reopened).unwrap();
}
#[test]
fn native_rejects_oversize_and_malformed_inputs() {
    use feishin_storage::{
        imports::{MAX_INPUT_BYTES, SettingsCategory},
        session::{OwnedInputs, SessionError},
    };
    let dir = tempfile::tempdir().unwrap();
    let mut w = NativeWorker::open(
        SessionGeneration(NonZeroU64::new(12).unwrap()),
        dir.path().into(),
        1,
    )
    .unwrap();
    let mut input = OwnedInputs {
        auth: Some(vec![0; MAX_INPUT_BYTES]),
        ..Default::default()
    };
    input
        .split_categories
        .insert(SettingsCategory::General, vec![0]);
    assert!(matches!(
        w.submit(Command::PrepareStage(input), Duration::from_secs(1)),
        Err(WorkerError::Invalid(SessionError::TooLarge))
    ));
    w.submit(Command::PrepareSave(b"{".to_vec()), Duration::from_secs(10))
        .unwrap();
    assert!(matches!(
        receive(&mut w).result,
        Err(SessionError::InvalidSave)
    ));
    w.begin_shutdown();
    joined(&mut w).unwrap();
}
#[test]
fn factory_panic_and_non_send_store() {
    use feishin_storage::{AtomicStore, Error, NativeStore};
    let mut failed = NativeWorker::with_factory::<NativeStore, NativeStore, NativeStore, _>(
        SessionGeneration(NonZeroU64::new(13).unwrap()),
        1,
        || panic!("factory panic"),
    )
    .unwrap();
    assert!(matches!(joined(&mut failed), Err(WorkerError::Panicked)));
    struct Local(std::rc::Rc<()>);
    impl AtomicStore for Local {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            let _ = &self.0;
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    let caller_thread = std::thread::current().id();
    let mut worker = NativeWorker::with_factory(
        SessionGeneration(NonZeroU64::new(14).unwrap()),
        1,
        move || {
            assert_ne!(std::thread::current().id(), caller_thread);
            Ok((
                Local(std::rc::Rc::new(())),
                Local(std::rc::Rc::new(())),
                Local(std::rc::Rc::new(())),
            ))
        },
    )
    .unwrap();
    assert!(matches!(
        execute(&mut worker, Command::Load),
        Response::Load(_)
    ));
    worker.begin_shutdown();
    joined(&mut worker).unwrap();
}

#[test]
fn queued_expiry_and_shutdown_without_polling() {
    use feishin_storage::{AtomicStore, Error, session::SessionError};
    use std::sync::mpsc;
    struct Gate {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl AtomicStore for Gate {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    struct Empty;
    impl AtomicStore for Empty {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = NativeWorker::with_factory(
        SessionGeneration(NonZeroU64::new(15).unwrap()),
        2,
        move || {
            Ok((
                Gate {
                    entered: entered_tx,
                    release: release_rx,
                },
                Empty,
                Empty,
            ))
        },
    )
    .unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    w.submit(Command::Load, Duration::ZERO).unwrap();
    release_tx.send(()).unwrap();
    assert!(matches!(receive(&mut w).result, Ok(Response::Load(_))));
    assert!(matches!(receive(&mut w).result, Err(SessionError::Expired)));
    w.begin_shutdown();
    joined(&mut w).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut w = NativeWorker::open(
        SessionGeneration(NonZeroU64::new(16).unwrap()),
        dir.path().into(),
        2,
    )
    .unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    w.begin_shutdown();
    joined(&mut w).unwrap();
    assert!(w.poll().unwrap().is_some());
    assert!(w.poll().unwrap().is_some());
}

#[test]
fn completed_unread_replies_count_toward_capacity() {
    use feishin_storage::{AtomicStore, Error};
    use std::sync::mpsc;
    struct Signal(mpsc::Sender<()>);
    impl AtomicStore for Signal {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            self.0.send(()).unwrap();
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    struct Empty;
    impl AtomicStore for Empty {
        fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            Ok(None)
        }
        fn replace(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }
    let (tx, rx) = mpsc::channel();
    let mut w = NativeWorker::with_factory(
        SessionGeneration(NonZeroU64::new(17).unwrap()),
        2,
        move || Ok((Signal(tx), Empty, Empty)),
    )
    .unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        w.submit(Command::Load, Duration::from_secs(10)),
        Err(WorkerError::Full)
    ));
    receive(&mut w);
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    w.begin_shutdown();
    joined(&mut w).unwrap();
}
#[test]
fn factory_failure_closes_admission_with_pending_request() {
    use feishin_storage::{Error, NativeStore};
    use std::sync::mpsc;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = NativeWorker::with_factory::<NativeStore, NativeStore, NativeStore, _>(
        SessionGeneration(NonZeroU64::new(18).unwrap()),
        1,
        move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Err(Error::Io)
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    w.submit(Command::Load, Duration::from_secs(10)).unwrap();
    release_tx.send(()).unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match w.poll() {
            Err(WorkerError::Closed) => break,
            Ok(None) => {
                assert!(std::time::Instant::now() < end);
                std::thread::yield_now();
            }
            _ => panic!("unexpected reply"),
        }
    }
    assert!(matches!(
        w.submit(Command::Load, Duration::from_secs(10)),
        Err(WorkerError::Closed)
    ));
    assert!(matches!(
        joined(&mut w),
        Err(WorkerError::Factory(Error::Io))
    ));
}
