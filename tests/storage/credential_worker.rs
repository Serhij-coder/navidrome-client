#![cfg(not(target_arch = "wasm32"))]
use feishin_domain::Secret;
use feishin_storage::{
    BackendKind, Profile,
    credential_executor::{Command, DispatchError, Generation, Response},
    credential_worker::{CredentialWorker, WorkerError},
    credentials::{
        CredentialError, CredentialKey, CredentialSlot, MAX_SECRET_BYTES, NoPersistence,
        PersistentCredentials,
    },
};
use std::{
    num::NonZeroU64,
    sync::mpsc,
    time::{Duration, Instant},
};
fn generation() -> Generation {
    Generation(NonZeroU64::new(1).unwrap())
}
fn key() -> CredentialKey {
    CredentialKey::new(
        "test",
        &Profile {
            id: "id".into(),
            name: "name".into(),
            endpoint: "https://example.test".into(),
            backend: BackendKind::Subsonic,
        },
        "account",
        CredentialSlot::LoginPassword,
    )
    .unwrap()
}
fn poll(w: &mut CredentialWorker) -> feishin_storage::credential_executor::Reply {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(r) = w.poll().unwrap() {
            return r;
        }
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
}
struct Block {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}
impl PersistentCredentials for Block {
    fn persist(&mut self, _: &CredentialKey, _: &Secret) -> Result<(), CredentialError> {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        Ok(())
    }
    fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
        Ok(None)
    }
    fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
        Ok(())
    }
}
#[test]
fn admission_correlation_and_shutdown_gates() {
    assert!(matches!(
        CredentialWorker::with_factory(generation(), 0, || Ok(NoPersistence)),
        Err(WorkerError::InvalidCapacity)
    ));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = CredentialWorker::with_factory(generation(), 2, move || {
        Ok(Block {
            entered: entered_tx,
            release: release_rx,
        })
    })
    .unwrap();
    assert!(matches!(
        w.submit(
            Command::Put(key(), Secret::new(vec![0; MAX_SECRET_BYTES + 1])),
            Duration::from_secs(1)
        ),
        Err(WorkerError::Invalid(DispatchError::TooLarge))
    ));
    let put = w
        .submit(
            Command::Put(key(), Secret::new(vec![1])),
            Duration::from_secs(10),
        )
        .unwrap();
    assert_eq!(poll(&mut w).id, put);
    let active = w
        .submit(Command::Remember(key()), Duration::from_secs(10))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let queued = w
        .submit(Command::Get(key()), Duration::from_secs(10))
        .unwrap();
    assert!(matches!(
        w.submit(Command::Clear, Duration::from_secs(10)),
        Err(WorkerError::Full)
    ));
    assert!(w.cancel(generation(), active));
    assert!(w.cancel(generation(), queued));
    w.begin_shutdown();
    assert!(w.try_join().is_none());
    release_tx.send(()).unwrap();
    let first = poll(&mut w);
    assert_eq!(first.id, active);
    assert!(matches!(first.result, Ok(Response::Done)));
    let second = poll(&mut w);
    assert_eq!(second.id, queued);
    assert!(matches!(second.result, Err(DispatchError::Cancelled)));
}
#[test]
fn unread_reply_holds_credit_and_expiration() {
    let mut w = CredentialWorker::with_factory(generation(), 1, || Ok(NoPersistence)).unwrap();
    let id = w.submit(Command::Clear, Duration::ZERO).unwrap();
    assert!(matches!(
        w.submit(Command::Clear, Duration::from_secs(1)),
        Err(WorkerError::Full)
    ));
    let reply = poll(&mut w);
    assert_eq!(reply.id, id);
    assert!(matches!(reply.result, Err(DispatchError::Expired)));
}
#[test]
fn factory_failure_and_panic() {
    for panic in [false, true] {
        let mut w =
            CredentialWorker::with_factory::<NoPersistence, _>(generation(), 1, move || {
                if panic {
                    panic!("synthetic");
                }
                Err(CredentialError::Unavailable)
            })
            .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = w.try_join() {
                assert!(if panic {
                    matches!(result, Err(WorkerError::Panicked))
                } else {
                    matches!(
                        result,
                        Err(WorkerError::Factory(CredentialError::Unavailable))
                    )
                });
                break;
            }
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
    }
}

#[test]
fn factory_can_construct_non_send_provider_and_default_is_session_only() {
    struct Local(std::rc::Rc<()>);
    impl PersistentCredentials for Local {
        fn persist(&mut self, _: &CredentialKey, _: &Secret) -> Result<(), CredentialError> {
            let _ = &self.0;
            Err(CredentialError::Unsupported)
        }
        fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
            Err(CredentialError::Unsupported)
        }
        fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
            Ok(())
        }
    }
    let mut local =
        CredentialWorker::with_factory(generation(), 1, || Ok(Local(std::rc::Rc::new(()))))
            .unwrap();
    local
        .submit(Command::Clear, Duration::from_secs(5))
        .unwrap();
    assert!(matches!(poll(&mut local).result, Ok(Response::Done)));
    let mut default = CredentialWorker::new(generation(), 1).unwrap();
    default
        .submit(
            Command::Put(key(), Secret::new(vec![1])),
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(matches!(poll(&mut default).result, Ok(Response::Done)));
    default
        .submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        poll(&mut default).result,
        Err(DispatchError::MutationUncertain(
            CredentialError::Unsupported
        ))
    ));
    default
        .submit(Command::Get(key()), Duration::from_secs(5))
        .unwrap();
    assert!(
        matches!(poll(&mut default).result, Ok(Response::Secret(Some(s))) if s.expose() == [1])
    );
    default
        .submit(Command::Restore(key()), Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        poll(&mut default).result,
        Err(DispatchError::Operation(CredentialError::Unsupported))
    ));
    default
        .submit(Command::Forget(key()), Duration::from_secs(5))
        .unwrap();
    assert!(matches!(poll(&mut default).result, Ok(Response::Done)));
    default
        .submit(Command::Get(key()), Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        poll(&mut default).result,
        Ok(Response::Secret(None))
    ));
}

#[test]
fn queued_expiration_and_shutdown_with_unread_replies() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = CredentialWorker::with_factory(generation(), 2, move || {
        Ok(Block {
            entered: entered_tx,
            release: release_rx,
        })
    })
    .unwrap();
    w.submit(
        Command::Put(key(), Secret::new(vec![1])),
        Duration::from_secs(5),
    )
    .unwrap();
    poll(&mut w);
    w.submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    w.submit(Command::Get(key()), Duration::ZERO).unwrap();
    w.begin_shutdown();
    release_tx.send(()).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = w.try_join() {
            assert!(result.is_ok());
            break;
        }
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(matches!(poll(&mut w).result, Ok(Response::Done)));
    // Shutdown cancellation has precedence; expiry is separately tested below.
    assert!(matches!(poll(&mut w).result, Err(DispatchError::Cancelled)));
}

#[test]
fn queued_expiration_behind_blocked_provider() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = CredentialWorker::with_factory(generation(), 2, move || {
        Ok(Block {
            entered: entered_tx,
            release: release_rx,
        })
    })
    .unwrap();
    w.submit(
        Command::Put(key(), Secret::new(vec![1])),
        Duration::from_secs(5),
    )
    .unwrap();
    poll(&mut w);
    w.submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    w.submit(Command::Clear, Duration::ZERO).unwrap();
    release_tx.send(()).unwrap();
    assert!(matches!(poll(&mut w).result, Ok(Response::Done)));
    assert!(matches!(poll(&mut w).result, Err(DispatchError::Expired)));
}

#[test]
fn drop_detaches_blocked_provider_and_eventually_cleans_up() {
    struct DropBlock {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
        dropped: mpsc::Sender<()>,
    }
    impl Drop for DropBlock {
        fn drop(&mut self) {
            self.dropped.send(()).unwrap();
        }
    }
    impl PersistentCredentials for DropBlock {
        fn persist(&mut self, _: &CredentialKey, _: &Secret) -> Result<(), CredentialError> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(())
        }
        fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
            Ok(None)
        }
        fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let mut w = CredentialWorker::with_factory(generation(), 1, move || {
        Ok(DropBlock {
            entered: entered_tx,
            release: release_rx,
            dropped: dropped_tx,
        })
    })
    .unwrap();
    w.submit(
        Command::Put(key(), Secret::new(vec![1])),
        Duration::from_secs(5),
    )
    .unwrap();
    poll(&mut w);
    w.submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(w);
    release_tx.send(()).unwrap();
    dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn completed_unread_reply_keeps_credit() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut w = CredentialWorker::with_factory(generation(), 2, move || {
        Ok(Block {
            entered: entered_tx,
            release: release_rx,
        })
    })
    .unwrap();
    // Entering the provider proves the preceding put reply has been sent.
    w.submit(
        Command::Put(key(), Secret::new(vec![1])),
        Duration::from_secs(5),
    )
    .unwrap();
    w.submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        w.submit(Command::Clear, Duration::from_secs(5)),
        Err(WorkerError::Full)
    ));
    assert!(matches!(poll(&mut w).result, Ok(Response::Done)));
    release_tx.send(()).unwrap();
    assert!(matches!(poll(&mut w).result, Ok(Response::Done)));
}

#[test]
fn provider_panic_after_mutation_has_no_reply_and_requires_reconciliation() {
    struct PanicAfter(mpsc::Sender<()>);
    impl PersistentCredentials for PanicAfter {
        fn persist(&mut self, _: &CredentialKey, _: &Secret) -> Result<(), CredentialError> {
            self.0.send(()).unwrap();
            panic!("synthetic post-mutation panic")
        }
        fn load(&mut self, _: &CredentialKey) -> Result<Option<Secret>, CredentialError> {
            Ok(None)
        }
        fn delete(&mut self, _: &CredentialKey) -> Result<(), CredentialError> {
            Ok(())
        }
    }
    let (tx, rx) = mpsc::channel();
    let mut w =
        CredentialWorker::with_factory(generation(), 1, move || Ok(PanicAfter(tx))).unwrap();
    let id = w
        .submit(
            Command::Put(key(), Secret::new(vec![1])),
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(!w.cancel(Generation(NonZeroU64::new(2).unwrap()), id));
    let reply = poll(&mut w);
    assert_eq!(reply.generation, generation());
    let id = w
        .submit(Command::Remember(key()), Duration::from_secs(5))
        .unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = w.try_join() {
            assert!(matches!(result, Err(WorkerError::Panicked)));
            break;
        }
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(matches!(w.poll(), Err(WorkerError::Closed)));
    assert!(w.cancel(generation(), id)); // request remains unresolved, not proof of cancellation
}
